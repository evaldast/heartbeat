use std::io;
use std::time::Duration;

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use termion::event::Key;
use termion::input::MouseTerminal;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::Color;
use tui::widgets::canvas::{Canvas, Points};
use tui::widgets::{Block, Borders, Widget};
use tui::Terminal;

pub enum Event<I> {
    Input(I),
    Tick,
}

pub struct Events {
    rx: mpsc::Receiver<Event<Key>>,
    input_handle: thread::JoinHandle<()>,
    tick_handle: thread::JoinHandle<()>,
}

impl Events {
    pub fn init(tick_rate: Duration) -> Events {
        let (tx, rx) = mpsc::channel();
        let input_handle = {
            let tx = tx.clone();
            thread::spawn(move || {
                let stdin = io::stdin();
                for evt in stdin.keys() {
                    if let Ok(key) = evt {
                        if tx.send(Event::Input(key)).is_err() {
                            return;
                        }
                    }
                }
            })
        };
        let tick_handle = {
            let tx = tx.clone();
            thread::spawn(move || {
                let tx = tx.clone();
                loop {
                    tx.send(Event::Tick).unwrap();
                    thread::sleep(tick_rate);
                }
            })
        };
        Events {
            rx,
            input_handle,
            tick_handle,
        }
    }

    pub fn next(&self) -> Result<Event<Key>, mpsc::RecvError> {
        self.rx.recv()
    }
}

type Test = Arc<Mutex<bool>>;

struct App {
    line: Vec<(f64, f64)>,
    dot_x: f64,
    dot_y: f64,
    initial_heartbeat_x: f64,
    is_beating: bool,
    test: Test,
}

impl App {
    fn new() -> App {
        let mut dots: Vec<(f64, f64)> = vec![];

        for _ in 0..=1000 {
            dots.push((0.0, 0.0))
        }

        App {
            line: dots,
            dot_x: 0.0,
            dot_y: 500.0,
            initial_heartbeat_x: 0.0,
            is_beating: false,
            test: Arc::new(Mutex::new(false)),
        }
    }

    fn update(&mut self) {
        fn heartbeat(state: &mut App) {
            if state.dot_x > state.initial_heartbeat_x
                && state.dot_x <= state.initial_heartbeat_x + 20.0
            {
                state.dot_y += 15.0;

                for x in 0..15 {
                    state.line[((state.dot_x as usize) + x) % 1000] = (state.dot_x, state.dot_y + x as f64);
                }
            }

            if state.dot_x > state.initial_heartbeat_x + 20.0
                && state.dot_x <= state.initial_heartbeat_x + 50.0
            {
                state.dot_y -= 15.0;

                for x in 0..15 {
                    state.line[((state.dot_x as usize) + x) % 1000] = (state.dot_x, state.dot_y - x as f64);
                }
            }

            if state.dot_x > state.initial_heartbeat_x + 50.0
                && state.dot_x <= state.initial_heartbeat_x + 60.0
            {
                state.dot_y += 15.0;

                for x in 0..15 {
                    state.line[((state.dot_x as usize) + x) % 1000] = (state.dot_x, state.dot_y + x as f64);
                }
            }

            if state.dot_x > state.initial_heartbeat_x + 60.0 {
                state.is_beating = false;
            }
        }

        if self.dot_x >= 1000.0 {
            self.dot_x = 0.0;
            self.dot_y = 500.0;
        }

        self.line[self.dot_x as usize] = (self.dot_x, self.dot_y);

        match self.test.lock() {
            Ok(guard) => {
                if (*guard) {
                    self.initial_heartbeat_x = self.dot_x;
                    self.is_beating = true;
                }
            }
            Err(_) => {}
        }

        if self.is_beating {
            match self.test.lock() {
                Ok(mut guard) => {
                    *guard = false;
                }
                Err(_) => {}
            }

            heartbeat(self);
        }

        let decay_index = match self.dot_x - 600.0 >= 0.0 {
            true => (self.dot_x - 600.0) as usize,
            false => (1000.0 + (self.dot_x - 600.0)) as usize,
        };

        self.line[decay_index] = (0.0, 0.0);

        self.dot_x += 1.0;
    }
}

fn main() -> Result<(), failure::Error> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.hide_cursor()?;

    let events = Events::init(Duration::from_millis(10));
    let mut app = App::new();

    start_ping_thread(app.test.clone());

    loop {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());

            Canvas::default()
                .block(Block::default().borders(Borders::ALL))
                .paint(|ctx| {
                    ctx.draw(&Points {
                        coords: &app.line,
                        color: Color::LightGreen,
                    });

                    ctx.draw(&Points {
                        coords: &[(app.dot_x, app.dot_y)],
                        color: Color::White,
                    });
                })
                .x_bounds([0.0, 1000.0])
                .y_bounds([0.0, 1000.0])
                .render(&mut f, chunks[0]);
        })?;

        match events.next()? {
            Event::Input(input) => match input {
                Key::Char('q') => {
                    panic!();
                }
                _ => {}
            },
            Event::Tick => {
                app.update();
            }
        }
    }
}

fn start_ping_thread(test: Test) {
    fn ping() -> Result<(), failure::Error> {
        let resp = reqwest::get("http://bonus-engine-api.bonus:5000/api/test/test")?.text()?;

        Ok(())
    }

    thread::spawn(move || loop {
        match ping() {
            Ok(_) => match test.lock() {
                Ok(mut guard) => {
                    *guard = true;
                }
                Err(_) => {}
            },
            Err(error) => {}
        }

        thread::sleep(std::time::Duration::from_secs(3));
    });
}
