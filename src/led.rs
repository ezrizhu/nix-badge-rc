// taken from https://github.com/esp-rs/std-training
use anyhow::Result;
use core::time::Duration;
use esp_idf_hal::{
    gpio::OutputPin,
    peripheral::Peripheral,
    rmt::{config::TransmitConfig, FixedLengthSignal, PinState, Pulse, RmtChannel, TxRmtDriver},
};

pub use rgb::RGB8;

pub struct WS2812RMT<'a> {
    tx_rtm_driver: TxRmtDriver<'a>,
}

impl<'d> WS2812RMT<'d> {
    pub fn new(
        led: impl Peripheral<P = impl OutputPin> + 'd,
        channel: impl Peripheral<P = impl RmtChannel> + 'd,
    ) -> Result<Self> {
        let config = TransmitConfig::new().clock_divider(2);
        let tx = TxRmtDriver::new(channel, led, &config)?;
        Ok(Self { tx_rtm_driver: tx })
    }

    pub fn set_pixels(&mut self, rgbs: &[RGB8]) -> Result<()> {
        if rgbs.is_empty() {
            return Ok(());
        }

        let ticks_hz = self.tx_rtm_driver.counter_clock()?;
        let t0h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(350))?;
        let t0l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(800))?;
        let t1h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(700))?;
        let t1l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(600))?;

        // Signal size = 24 bits per LED
        let mut signal = FixedLengthSignal::<{ 12 * 24 }>::new();

        for (led_index, rgb) in rgbs.iter().enumerate() {
            let color: u32 = ((rgb.g as u32) << 16) | ((rgb.r as u32) << 8) | rgb.b as u32;

            for i in (0..24).rev() {
                let p = 2_u32.pow(i);
                let bit = p & color != 0;
                let (high_pulse, low_pulse) = if bit { (t1h, t1l) } else { (t0h, t0l) };

                let signal_index = led_index * 24 + (23 - i as usize);
                signal.set(signal_index, &(high_pulse, low_pulse))?;
            }
        }

        self.tx_rtm_driver.start_blocking(&signal)?;
        Ok(())
    }
}

fn ns(nanos: u64) -> Duration {
    Duration::from_nanos(nanos)
}

pub fn hsv_to_rgb(h: u16, s: u8, v: u8) -> RGB8 {
    let h = h % 360;
    let s = s as f32 / 255.0;
    let v = v as f32 / 255.0;

    let c = v * s;
    let x = c * (1.0 - ((h as f32 / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match h / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    RGB8::new(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
