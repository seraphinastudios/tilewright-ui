// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
//! Opt-in private bounded acceptance evidence; never records by default.
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
};
use tilewright_app::{App, PresentationStamp};

pub struct Trace {
    file: File,
    records: usize,
    bytes: usize,
}
impl Trace {
    pub fn create(path: &str) -> io::Result<Self> {
        // create_new is exclusive and rejects existing files and symlinks.
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        if file.metadata()?.permissions().mode() & 0o777 != 0o600 {
            return Err(io::Error::other("trace permissions must be 0600"));
        }
        let mut trace = Self {
            file,
            records: 0,
            bytes: 0,
        };
        trace.line("twui-trace-v1".into())?;
        Ok(trace)
    }
    fn line(&mut self, mut value: String) -> io::Result<()> {
        value.push('\n');
        if self.records >= 16_384 || self.bytes + value.len() > 4 * 1024 * 1024 {
            return Err(io::Error::other("private trace budget exhausted"));
        }
        self.file.write_all(value.as_bytes())?;
        self.records += 1;
        self.bytes += value.len();
        Ok(())
    }
    pub fn event(
        &mut self,
        name: &str,
        rejected: bool,
        consumed: bool,
        app: &App,
        held: usize,
    ) -> io::Result<()> {
        let state = app.ui().snapshot();
        self.line(format!(
            "event\t{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{held}\t{}\t{}\t{}\t{}\t{}\t{}",
            u8::from(rejected),
            u8::from(consumed),
            hex(&state.composer.committed),
            hex(state
                .composer
                .composition
                .as_ref()
                .map_or("", |value| value.text.as_str())),
            state.control.epoch.get(),
            u8::from(state.control.world_input_allowed),
            u8::from(app.ui().text_target().is_some()),
            u8::from(state.control.modal_open),
            state.composer.selection.anchor.byte,
            state.composer.selection.focus.byte,
            state.scroll,
            state.messages.len()
        ))
    }
    pub fn action(&mut self, batch: u64, text: &str) -> io::Result<()> {
        self.line(format!("action\t{batch}\t{}", hex(text)))
    }
    pub fn present(&mut self, stamp: PresentationStamp) -> io::Result<()> {
        self.line(format!(
            "present\t{}\t{}\t{}\t{}",
            stamp.surface.get(),
            stamp.frame.get(),
            stamp.geometry.get(),
            stamp.ui_revision.get()
        ))
    }
}
fn hex(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 15)]));
    }
    output
}
pub fn event_name(event: &sdl3::event::Event) -> String {
    use sdl3::event::Event;
    match event {
        Event::KeyDown { scancode, .. } => {
            format!("key_down:{}", scancode.map_or(0, |code| code as u16))
        }
        Event::KeyUp { scancode, .. } => {
            format!("key_up:{}", scancode.map_or(0, |code| code as u16))
        }
        Event::TextInput { .. } => "text_input".into(),
        Event::TextEditing { .. } => "preedit".into(),
        Event::MouseButtonDown { .. } => "pointer_down".into(),
        Event::MouseWheel { .. } => "wheel".into(),
        Event::Window { win_event, .. } => format!("window:{win_event:?}"),
        Event::Quit { .. } => "quit".into(),
        _ => "other".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_is_exclusive_private_and_bounded() {
        let directory =
            std::env::temp_dir().join(format!("twui-private-trace-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("trace");
        let path = path.to_str().unwrap();
        let mut trace = Trace::create(path).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(Trace::create(path).is_err());
        trace.records = 16_384;
        assert!(trace.line("too many".into()).is_err());
        let symlink = directory.join("symlink");
        std::os::unix::fs::symlink(path, &symlink).unwrap();
        assert!(Trace::create(symlink.to_str().unwrap()).is_err());
        drop(trace);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
