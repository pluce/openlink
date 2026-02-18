use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use std::time::Duration;

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub fn init() -> io::Result<Tui> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
}

pub fn restore() -> io::Result<()> {
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    Ok(())
}

#[derive(Debug, Clone)]
pub enum Action {
    Tick,
    Quit,
    Resize(u16, u16),
    Key(event::KeyEvent),
    MessageReceived(String), 
    Error(String),
}

pub struct EventHandler {
    sender: mpsc::UnboundedSender<Action>,
    receiver: mpsc::UnboundedReceiver<Action>,
}

impl EventHandler {
    pub fn new(tick_rate: u64) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        // 1. Tick Loop (Async)
        let tick_sender = sender.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(tick_rate));
            loop {
                interval.tick().await;
                if tick_sender.send(Action::Tick).is_err() {
                    break;
                }
            }
        });

        // 2. Input Loop (Blocking Thread)
        let event_sender = sender.clone();
        std::thread::spawn(move || {
            loop {
                // Blocks until event available
                match event::read() {
                    Ok(Event::Key(key)) => {
                        if key.kind == KeyEventKind::Press {
                             if event_sender.send(Action::Key(key)).is_err() { break; }
                        }
                    },
                    Ok(Event::Resize(w, h)) => {
                         if event_sender.send(Action::Resize(w, h)).is_err() { break; }
                    },
                    Err(_) => {
                         // On error, we exit the input loop
                         break;
                    }
                    _ => {}
                }
            }
        });

        Self {
            sender,
            receiver,
        }
    }

    pub fn next(&mut self) -> Result<Action, mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }
    
    pub async fn next_async(&mut self) -> Option<Action> {
        self.receiver.recv().await
    }
    
    pub fn get_sender(&self) -> mpsc::UnboundedSender<Action> {
        self.sender.clone()
    }
}
