extern crate jack;
extern crate wmidi;
use crossbeam_channel::{unbounded, Receiver, Sender};
use itertools::izip;
use std::convert::TryFrom;
use std::sync::mpsc;
use std::{thread, time::Duration};
mod jackmidi;
use jackmidi::MidiMsg;

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
}

fn start_jack_thread(
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

            for (
                _idx,
                (
                    sample_in0_l,
                    sample_in0_r,
                    sample_in1_l,
                    sample_in1_r,
                    sample_out_l,
                    sample_out_r,
                ),
            ) in izip!(in0_l_p, in0_r_p, in1_l_p, in1_r_p, out_l_p, out_r_p).enumerate()
            {
                *sample_out_l = *sample_in0_l + *sample_in1_l;
                *sample_out_r = *sample_in0_r + *sample_in1_r;
            }

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
