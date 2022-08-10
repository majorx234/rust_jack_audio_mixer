extern crate jack;
extern crate wmidi;
use std::sync::mpsc;
use std::{thread, time::Duration};
mod jackmidi;
use jackmidi::MidiMsg;

fn main() {
    let (midi_sender, midi_receiver): (
        std::sync::mpsc::SyncSender<MidiMsg>,
        std::sync::mpsc::Receiver<MidiMsg>,
    ) = mpsc::sync_channel(64);
    let audio_thread = start_audio_thread(midi_sender);
    audio_thread.join().unwrap();
}

fn start_audio_thread(
    midi_sender: std::sync::mpsc::SyncSender<MidiMsg>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let (client, _status) = jack::Client::new(
            "rust_jack_audio_mixer",
            jack::ClientOptions::NO_START_SERVER,
        )
        .unwrap();
        let sample_rate = client.sample_rate();

        // register ports
        // in ports
        let in0_l = client
            .register_port("mixer_in0_l", jack::AudioIn::default())
            .unwrap();
        let in0_r = client
            .register_port("mixer_in0_r", jack::AudioIn::default())
            .unwrap();
        // register ports
        let in1_l = client
            .register_port("mixer_in1_l", jack::AudioIn::default())
            .unwrap();
        let in1_r = client
            .register_port("mixer_in1_r", jack::AudioIn::default())
            .unwrap();

        // out ports
        let mut out_l = client
            .register_port("mixer_out_l", jack::AudioOut::default())
            .unwrap();
        let mut out_r = client
            .register_port("mixer_out_r", jack::AudioOut::default())
            .unwrap();

        // midi_in ports
        let midi_in = client
            .register_port("mixer_midi_in", jack::MidiIn::default())
            .unwrap();

        let frame_size = client.buffer_size();

        let process_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
            let show_p = midi_in.iter(ps);
            for e in show_p {
                let c: MidiMsg = e.into();
                let _ = midi_sender.try_send(c);
            }

            let out_l_p = out_l.as_mut_slice(ps);
            let out_r_p = out_r.as_mut_slice(ps);

            let in0_l_p = in0_l.as_slice(ps);
            let in0_r_p = in0_r.as_slice(ps);
            let in1_l_p = in1_l.as_slice(ps);
            let in1_r_p = in1_r.as_slice(ps);

            out_l_p.clone_from_slice(in0_l_p);
            out_r_p.clone_from_slice(in0_r_p);

            jack::Control::Continue
        };

        let process = jack::ClosureProcessHandler::new(process_callback);

        // Activate the client, which starts the processing.
        let active_client = client.activate_async((), process).unwrap();
        let mut run: bool = true;
        while run {
            thread::sleep(Duration::from_millis(100));
            //    run = rx_close.recv().unwrap();
        }
        println!("exit audio thread\n");
        active_client.deactivate().unwrap();
    })
}
