use crate::vm::{Action, Const, Reg, VM};
use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use std::io::stdout;

pub trait TerminalExt {
    fn print_state(&self) -> Result<()>;
    fn interactive(&mut self) -> Result<()>;
}

impl TerminalExt for VM {
    fn print_state(&self) -> Result<()> {
        use crossterm::style::*;
        execute!(
            stdout(),
            cursor::MoveTo(0, 0),
            SetForegroundColor(Color::Yellow)
        )?;
        for i in 0..16 {
            execute!(stdout(), Print(format!("R{i:X} ")),)?;
        }
        execute!(stdout(), cursor::MoveToNextLine(1), ResetColor,)?;
        for i in 0..16 {
            if matches!(self.actions.last(), Some(Action::SetReg(Reg(j), _)) if i == *j) {
                execute!(stdout(), SetBackgroundColor(Color::DarkMagenta))?;
            }
            execute!(
                stdout(),
                Print(format!("{:02X}", self.getr(Reg(i)).0)),
                ResetColor,
                Print(' '),
            )?;
        }
        execute!(stdout(), ResetColor, cursor::MoveToNextLine(1))?;
        let s = format!("{:?}", self.dis(self.pc));
        let index = s.find('(').unwrap_or(s.len());
        execute!(
            stdout(),
            cursor::MoveToNextLine(1),
            SetForegroundColor(Color::DarkGrey),
            Clear(ClearType::CurrentLine),
            Print("Current: "),
            SetForegroundColor(Color::Yellow),
            Print(&s[..index]),
            SetForegroundColor(Color::Red),
            Print(&s[index..]),
            ResetColor,
            cursor::MoveToNextLine(1)
        )?;
        for i in 0..=255 {
            if i % 16 == 0 {
                execute!(
                    stdout(),
                    cursor::MoveToNextLine(1),
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!("0x{i:02X}:")),
                    ResetColor,
                )?;
            }
            execute!(stdout(), Print(' '))?;
            if matches!(self.actions.last(), Some(Action::SetMem(Const(j), _)) if i == *j) {
                execute!(stdout(), SetBackgroundColor(Color::DarkMagenta))?;
            }
            if i == self.pc.0 {
                execute!(stdout(), SetBackgroundColor(Color::Blue))?;
            }
            execute!(
                stdout(),
                Print(format!("{:02X}", self.memory[i as usize])),
                ResetColor
            )?;
        }
        execute!(stdout(), cursor::MoveToNextLine(2))?;
        for (key, desc) in [
            ("Q", "Quit"),
            ("S", "Step"),
            ("Z", "Redo"),
            ("R", "Reset"),
            ("Enter", "Run All"),
        ] {
            execute!(
                stdout(),
                SetBackgroundColor(Color::DarkGreen),
                SetForegroundColor(Color::White),
                Print(format!("[{key}]")),
                ResetColor,
                SetForegroundColor(Color::DarkGreen),
                Print(format!(" {desc} ")),
                ResetColor,
            )?;
        }
        execute!(stdout(), cursor::MoveToNextLine(1))?;
        Ok(())
    }

    fn interactive(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(stdout(), cursor::Hide, EnterAlternateScreen)?;
        fn inner(vm: &mut VM) -> Result<()> {
            loop {
                vm.print_state()?;
                if let Event::Key(event) = event::read()? {
                    match event.code {
                        KeyCode::Enter => {
                            while vm.step()? {}
                            break;
                        }
                        KeyCode::Char(c) => match c {
                            's' => {
                                if !vm.step()? {
                                    break;
                                }
                            }
                            'q' => {
                                break;
                            }
                            'r' => {
                                vm.reset();
                            }
                            'z' => {
                                vm.undo();
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
            Ok(())
        }
        let res = inner(self);
        self.print_state()?;
        execute!(stdout(), cursor::Show, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        res
    }
}
