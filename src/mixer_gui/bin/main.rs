extern crate jack;
extern crate wmidi;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::convert::TryFrom;
use std::sync::mpsc;
mod jackmidi;
use jackmidi::MidiMsg;
mod jackprocess;
use jackprocess::start_jack_thread;

fn main() {
    let (tx_close, rx1_close) = unbounded();
    let rx2_close = rx1_close.clone();
    let (midi_sender, midi_receiver): (
        std::sync::mpsc::SyncSender<MidiMsg>,
        std::sync::mpsc::Receiver<MidiMsg>,
    ) = mpsc::sync_channel(64);
    let (tx_mix, rx_mix): (
        std::sync::mpsc::Sender<(u8, f32)>,
        std::sync::mpsc::Receiver<(u8, f32)>,
    ) = mpsc::channel();

    // mididata processing:
    let midi_thread: std::thread::JoinHandle<()> = std::thread::spawn(move || {
        while let Ok(m) = midi_receiver.recv() {
            let bytes: &[u8] = &m.data;
            let message = wmidi::MidiMessage::try_from(bytes);
            if let Ok(wmidi::MidiMessage::ControlChange(channel, controlNumber, controlValue)) =
                message
            {
                let volume = u8::from(controlValue) as f32 / 127.0;
                let mix_channel = u8::from(controlNumber);
                tx_mix.send((mix_channel, volume)).unwrap();
            }

            println!("{:?}", m);
            let mut run = true;
            run = rx1_close.try_recv().unwrap();
            if !run {
                break;
            }
        }
        println!("exit midi thread\n");
    });

    let audio_thread = start_jack_thread(midi_sender);
    audio_thread.join().unwrap();
    midi_thread.join().unwrap();
}
