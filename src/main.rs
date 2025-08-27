use embassy_executor::{main, Spawner};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_idf_hal::gpio::Gpio3;
use esp_idf_hal::gpio::Input;
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct PersonId {
    pub id: u32,
}

#[derive(Debug, Deserialize)]
pub struct CheckInRecord {
    pub person: PersonId,
}

pub type CheckInData = Vec<CheckInRecord>;

mod led;
use led::{hsv_to_rgb, RGB8, WS2812RMT};
mod wifi;
use wifi::wifi;
mod client;

static LED_CONTROL: Mutex<CriticalSectionRawMutex, Option<WS2812RMT>> = Mutex::new(None);

async fn spin_leds(color: RGB8) {
    let mut led_control = LED_CONTROL.lock().await;
    if let Some(led) = led_control.as_mut() {
        for _ in 0..3 {
            for position in 0..12 {
                let mut colors = [RGB8::new(0, 0, 0); 12];

                colors[position] = color;

                led.set_pixels(&colors).unwrap();
                Timer::after(Duration::from_millis(30)).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn button_task(button: PinDriver<'static, Gpio3, Input>) {
    let mut last_state = button.is_high();

    loop {
        Timer::after(Duration::from_millis(50)).await;
        let current_state = button.is_high();

        // rising edge
        if current_state && !last_state {
            log::info!("button pressed");
            let color = hsv_to_rgb(270, 255, 100);
            spin_leds(color).await;
        }
        last_state = current_state;
    }
}

#[embassy_executor::task]
async fn ambient_color_task() {
    let mut hue_offset = 0u16;
    let mut pulse_counter = 0u32;

    loop {
        let mut current_colors = [RGB8::new(0, 0, 0); 12];
        let pulse_angle = (pulse_counter as f32 * 0.0628).sin();
        let pulse_factor = (pulse_angle + 1.0) / 2.0;
        let min_brightness = 15.0;
        let max_brightness = 55.0;
        let brightness = (min_brightness + pulse_factor * (max_brightness - min_brightness)) as u8;
        let saturation = 175;

        for i in 0..12 {
            let hue = (hue_offset + (i as u16 * 30)) % 360;
            current_colors[i] = hsv_to_rgb(hue, saturation, brightness);
        }

        {
            let mut led_opt = LED_CONTROL.lock().await;
            if let Some(led) = led_opt.as_mut() {
                led.set_pixels(&current_colors).unwrap();
            }
        }

        hue_offset = (hue_offset + 1) % 360;
        pulse_counter = pulse_counter.wrapping_add(1);
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
async fn checkin_task() {
    let mut last_checkins: HashSet<u32> = HashSet::<u32>::new();
    loop {
        Timer::after(Duration::from_millis(1000)).await;
        let curr_checkins = client::get().unwrap();

        let new_checkins: Vec<_> = curr_checkins.difference(&last_checkins).collect();
        if !new_checkins.is_empty() {
            // this is broken rn, when a lot or smth it doesnt do any colors
            for id in new_checkins {
                log::info!("new checkin: {:?}", id);
                let color = hsv_to_rgb(255, 0, 0);
                spin_leds(color).await;
            }
        }

        last_checkins = curr_checkins;
    }
}

#[main]
async fn main(spawner: Spawner) {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("Hello, world!");

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let button = PinDriver::input(pins.gpio3).unwrap();

    let led = WS2812RMT::new(pins.gpio14, peripherals.rmt.channel0).unwrap();
    {
        let mut led_control = LED_CONTROL.lock().await;
        *led_control = Some(led);
    }

    let sysloop = EspSystemEventLoop::take().unwrap();
    let _wifi = wifi(
        "Recurse Center",
        WIFIPSK,
        peripherals.modem,
        sysloop,
    )
    .unwrap();

    spawner.spawn(ambient_color_task()).unwrap();
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(checkin_task()).unwrap();

    log::info!("Main loop running...");
    loop {
        Timer::after(Duration::from_millis(1000)).await;
    }
}
