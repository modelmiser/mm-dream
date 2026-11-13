use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{event, keyboard, time, window, Length, Limits, Subscription};
use cosmic::iced_widget::shader;
use cosmic::{executor, Application, ApplicationExt, Element};
use std::time::Duration;

mod dream;
mod pipeline;
use dream::{v40_bars, V40Bars, FIELD_MODE, NUM_COMBOS, SNAKE};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::default().size_limits(Limits::NONE.min_width(400.0).min_height(300.0));
    cosmic::app::run::<App>(settings, ())?;
    Ok(())
}

const DT: f32 = 0.016;
const AUTO_CYCLE_SECS: f32 = 9.0;
const ZOOM_SECS: f32 = 9.0; // duration of zoom in/out
const ZOOM_DWELL: f32 = 2.0; // static dwell after zoom-in and before zoom-out

// Fold counts for rotozoom modes 0..3
const FOLD_COUNTS: [u32; 4] = [0, 2, 5, 10];

#[derive(PartialEq)]
enum ZoomState {
    In,
    DwellIn,
    Settled,
    DwellOut,
    Out,
}

struct App {
    core: Core,
    blend: f32,
    target: f32,
    elapsed: f32,
    combo: usize,
    cycle_timer: f32,
    settle_secs: f32,
    zoom_t: f32, // 0 = far away, 1 = at screen
    zoom_state: ZoomState,
    has_cycled: bool, // true after first combo advance (prevents immediate zoom-out)
    bars: V40Bars,
    flux_sum: f32,
    flux_frames: u32,
    speed_factor: f32,
    flags: u32,       // shader flags, see CRT_OFF
    fullscreen: bool, // current window mode
    fs_applied: bool, // the start-up fullscreen request has been sent
    start_scene: Option<usize>, // --scene N: the first scene after the intro (1-based on the command line)
}

// Shader flags (0.2.0).
const CRT_OFF: u32 = 2; // no barrel, scanlines, grille or vignette: the picture reaches every edge

#[derive(Debug, Clone, Copy)]
enum Key {
    Next,       // n or space: on to the next scene of the supercycle now
    Crt,        // c: CRT post-process on/off
    Fullscreen, // f: fullscreen <-> window
    Quit,       // q or Esc
}

fn on_event(e: cosmic::iced::Event, _status: event::Status, _id: window::Id) -> Option<Message> {
    let cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed { key, repeat: false, .. }) = e else {
        return None;
    };
    let k = match key.as_ref() {
        keyboard::Key::Character("n") | keyboard::Key::Character(" ") => Key::Next,
        keyboard::Key::Character("c") => Key::Crt,
        keyboard::Key::Character("f") => Key::Fullscreen,
        keyboard::Key::Character("q") | keyboard::Key::Named(keyboard::key::Named::Escape) => Key::Quit,
        _ => return None,
    };
    Some(Message::Key(k))
}

const BAR_NAMES: [&str; 5] = ["wave", "plasma", "bounce", "cascade", "field"];
const FOLD_NAMES: [&str; 4] = ["\u{2500}", "2-fold", "5-fold", "10-fold"];

// Maximum allowed flux — above this, animation auto-slows to prevent strobing.
// Flux = sum(|bar_delta|) / bar_width × fold_count, measured per frame.
const FLUX_LIMIT: f32 = 10.0;

impl App {
    fn advance_combo(&mut self) {
        // One line per combo, printed when it ends
        if self.flux_frames > 0 && self.combo < NUM_COMBOS {
            let (b, r) = SNAKE[self.combo];
            let avg = self.flux_sum / self.flux_frames as f32;
            let chaos = (avg.round() as usize).clamp(1, 10);
            let filled = "\u{25aa}".repeat(chaos);
            let empty = "\u{00b7}".repeat(10 - chaos);
            let flux_str = if avg > FLUX_LIMIT {
                format!("  \u{2591}{:.0}\u{2591}", avg)
            } else {
                format!("  {:.1}", avg)
            };
            eprintln!(
                "  {:>2}  {:<9} {:<10} {}{}{}",
                self.combo + 1,
                BAR_NAMES[b as usize],
                FOLD_NAMES[r as usize],
                filled,
                empty,
                flux_str,
            );
        }
        self.flux_sum = 0.0;
        self.flux_frames = 0;

        let new_combo = if self.combo == NUM_COMBOS {
            self.start_scene.take().unwrap_or(0)
        } else {
            (self.combo + 1) % (NUM_COMBOS + 1)
        };
        self.settle_secs = self.transition_cost(self.combo, new_combo);

        if new_combo == NUM_COMBOS {
            self.target = 1.0;
            // Smoothly transition bars to mode 0 so outro matches intro pattern
            self.bars.step_to(0, 0);
        } else if self.combo == NUM_COMBOS {
            let (bar, rz) = SNAKE[new_combo];
            self.bars.set_mode(bar);
            self.bars.start_rz_transition(rz);
            self.target = 0.0;
        } else {
            let (bar, rz) = SNAKE[new_combo];
            self.bars.step_to(bar, rz);
        }
        self.combo = new_combo;
    }

    /// Visual distance → transition duration (0.33s for smooth, up to 3.0s for jarring).
    fn transition_cost(&self, old: usize, new: usize) -> f32 {
        // To/from static: 1 second
        if old == NUM_COMBOS || new == NUM_COMBOS {
            return 1.0;
        }

        let (old_bar, old_rz) = SNAKE[old];
        let (new_bar, new_rz) = SNAKE[new];
        // Bars <-> field: a crossfade between two different pictures; give it time
        if (old_bar == FIELD_MODE) != (new_bar == FIELD_MODE) {
            return 4.0;
        }
        let old_fold = FOLD_COUNTS[old_rz as usize];
        let new_fold = FOLD_COUNTS[new_rz as usize];

        if old_fold == new_fold {
            // Bar-only: still a big visual change (amplitude, width, motion)
            return 2.0;
        }

        // Fold count changes: scale by visual distance
        let fold_diff = (old_fold as f32 - new_fold as f32).abs();
        let mut cost = 2.5 + fold_diff / 5.0 * 1.5;

        // Entering or leaving kaleidoscope (fold 0) is extra jarring
        if old_fold == 0 || new_fold == 0 {
            cost += 1.0;
        }

        cost.min(5.0)
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    Key(Key),
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "com.modelmiser.MmDream";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, _flags: ()) -> (Self, Task<Message>) {
        core.set_header_title("mm-dream".into());
        core.window.content_container = false;
        core.window.show_headerbar = false;
        eprintln!("mm-dream \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500} zoom in \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}");
        eprintln!("       \u{2500}\u{2500}\u{2500} still \u{2500}\u{2500}\u{2500}");
        let mut bars = v40_bars(0.0).width(Length::Fill).height(Length::Fill);
        // Command line: --windowed (do not start fullscreen), --no-crt (start with the CRT post-process off),
        // --scene N (start the supercycle at scene N of NUM_COMBOS; 17 is the field scene).
        let args: Vec<String> = std::env::args().collect();
        let windowed = args.iter().any(|a| a == "--windowed");
        let flags = if args.iter().any(|a| a == "--no-crt") { CRT_OFF } else { 0 };
        let start_scene = args
            .iter()
            .position(|a| a == "--scene")
            .and_then(|i| args.get(i + 1))
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|n| (1..=NUM_COMBOS).contains(n))
            .map(|n| n - 1);
        bars.set_flags(flags);
        (
            App {
                core,
                blend: 1.0,
                target: 1.0,
                elapsed: 0.0,
                combo: NUM_COMBOS,
                cycle_timer: 0.0,
                settle_secs: 1.0,
                zoom_t: 0.0,
                zoom_state: ZoomState::In,
                has_cycled: false,
                bars,
                flux_sum: 0.0,
                flux_frames: 0,
                speed_factor: 1.0,
                flags,
                fullscreen: !windowed,
                fs_applied: windowed,
                start_scene,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Key(k) => {
                match k {
                    Key::Next => {
                        // only from a settled scene: the cycle timer fires on the next tick
                        if self.zoom_state == ZoomState::Settled && !self.bars.transitioning() && self.blend == self.target {
                            self.cycle_timer = AUTO_CYCLE_SECS;
                        }
                    }
                    Key::Crt => self.flags ^= CRT_OFF,
                    Key::Quit => return cosmic::iced::exit(),
                    Key::Fullscreen => {
                        self.fullscreen = !self.fullscreen;
                        if let Some(id) = self.core().main_window_id() {
                            let mode = if self.fullscreen { window::Mode::Fullscreen } else { window::Mode::Windowed };
                            return window::set_mode(id, mode);
                        }
                    }
                }
                self.bars.set_flags(self.flags);
                return Task::none();
            }
            Message::Tick => {
                // Measure flux at full speed (before limiter) for accurate chaos reading.
                // Only accumulate during steady state — skip transitions which spike
                // from position jumps between combos.
                let flux = self.bars.flux();
                let settled = self.zoom_state == ZoomState::Settled
                    && !self.bars.transitioning()
                    && self.blend == self.target;
                if settled {
                    self.flux_sum += flux;
                    self.flux_frames += 1;
                }

                // Speed limiter — cap visual change rate for seizure safety.
                // Use undampened flux (flux / speed_factor) to compute the target,
                // so the limiter knows the mode's true speed and doesn't oscillate.
                let undampened = flux / self.speed_factor;
                let target_factor = if undampened > FLUX_LIMIT {
                    FLUX_LIMIT / undampened
                } else {
                    1.0
                };
                // Fast attack (instant clamp), slow release (gradual ramp-up).
                if target_factor < self.speed_factor {
                    self.speed_factor = target_factor;
                } else {
                    self.speed_factor = self.speed_factor * 0.95 + target_factor * 0.05;
                }
                // (In the field scene the bars sit still, so the flux is 0 and the limiter releases to full speed.)
                self.elapsed = (self.elapsed + DT * self.speed_factor) % 10_000.0;

                // Zoom state machine — raw progress, eased before sending to GPU
                match self.zoom_state {
                    ZoomState::In => {
                        self.zoom_t = (self.zoom_t + DT / ZOOM_SECS).min(1.0);
                        if self.zoom_t >= 1.0 {
                            self.zoom_state = ZoomState::DwellIn;
                            self.cycle_timer = 0.0;
                        }
                    }
                    ZoomState::DwellIn => {
                        self.cycle_timer += DT;
                        if self.cycle_timer >= ZOOM_DWELL {
                            self.zoom_state = ZoomState::Settled;
                            // Pre-load timer so first advance fires immediately
                            self.cycle_timer = AUTO_CYCLE_SECS;
                        }
                    }
                    ZoomState::DwellOut => {
                        self.cycle_timer += DT;
                        if self.cycle_timer >= ZOOM_DWELL {
                            self.zoom_state = ZoomState::Out;
                            eprintln!("mm-dream \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500} zoom out \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}");
                        }
                    }
                    ZoomState::Out => {
                        self.zoom_t = (self.zoom_t - DT / ZOOM_SECS).max(0.0);
                        if self.zoom_t <= 0.0 {
                            self.zoom_state = ZoomState::In;
                            self.combo = NUM_COMBOS;
                            self.blend = 1.0;
                            self.target = 1.0;
                            self.bars.reset_modes();
                            eprintln!("mm-dream \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500} zoom in \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}");
                            eprintln!("       \u{2500}\u{2500}\u{2500} still \u{2500}\u{2500}\u{2500}");
                        }
                    }
                    ZoomState::Settled => {
                        let step = DT / self.settle_secs;
                        if self.blend < self.target {
                            self.blend = (self.blend + step).min(self.target);
                        } else if self.blend > self.target {
                            self.blend = (self.blend - step).max(self.target);
                        }
                        self.bars.advance_all_transitions(step);
                        // Auto-cycle — only advance when fully settled
                        if self.bars.auto_cycle()
                            && !self.bars.transitioning()
                            && self.blend == self.target
                        {
                            // After full cycle, zoom out immediately (no 9s dwell)
                            if self.has_cycled && self.combo == NUM_COMBOS {
                                eprintln!("       \u{2500}\u{2500}\u{2500} still \u{2500}\u{2500}\u{2500}");
                                self.zoom_state = ZoomState::DwellOut;
                                self.has_cycled = false;
                                self.cycle_timer = 0.0;
                            } else {
                                self.cycle_timer += DT;
                                if self.cycle_timer >= AUTO_CYCLE_SECS {
                                    self.advance_combo();
                                    self.has_cycled = true;
                                    self.cycle_timer = 0.0;
                                }
                            }
                        }
                    }
                }
            }
        }
        self.bars.set_time(self.elapsed);
        self.bars.set_blend(smoothstep(self.blend));
        // Ease-out in both directions: fast at far end, slow near screen
        let eased_zoom = smoothstep(self.zoom_t);
        self.bars.set_zoom_t(eased_zoom);
        if !self.fs_applied {
            if let Some(id) = self.core().main_window_id() {
                self.fs_applied = true;
                return Task::batch([
                    self.set_window_title("mm-dream".into(), id),
                    window::set_mode(id, window::Mode::Fullscreen),
                ]);
            }
        }
        // Set window title once (task switcher / Wayland xdg_toplevel)
        if self.flux_frames <= 1 {
            if let Some(id) = self.core().main_window_id() {
                return self.set_window_title("mm-dream".into(), id);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        shader::Shader::new(&self.bars)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(Duration::from_millis(16)).map(|_| Message::Tick),
            event::listen_with(on_event),
        ])
    }
}
