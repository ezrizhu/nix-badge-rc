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

async fn spin_leds(colors: &[RGB8]) {
    let mut led_control = LED_CONTROL.lock().await;
    if let Some(led) = led_control.as_mut() {
        let color_count = colors.len().min(12);

        for _ in 0..3 {
            for offset in 0..12 {
                let mut led_colors = [RGB8::new(0, 0, 0); 12];

                for (i, &color) in colors.iter().take(color_count).enumerate() {
                    let position = (i + offset) % 12;
                    led_colors[position] = color;
                }

                led.set_pixels(&led_colors).unwrap();
                Timer::after(Duration::from_millis(30)).await;
            }
        }
    }
}

fn user_colors(input: u32) -> Vec<RGB8> {
    let offsets = [0, 3333, 6666];
   
    let mut colors = Vec::new();
    
    for (_i, &offset) in offsets.iter().enumerate() {
        let offset_input = (input + offset) % 9999;
        
        // 0-359
        let h = (offset_input * 359 / 9999) as u16;
        // 150-255
        let s = (150 + (offset_input * (255 - 150) / 9999)) as u8;
        // 15-55
        let v = (15 + (offset_input * (55 - 15) / 9999)) as u8;
        
        colors.push(hsv_to_rgb(h, s, v));
    }
    
    colors
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
            let color1 = hsv_to_rgb(196, 175, 20); //tran
            let color2 = hsv_to_rgb(0, 0, 20); //sgen
            let color3 = hsv_to_rgb(348, 175, 20); //der
            spin_leds(&[color1, color2, color3, color1, color2, color3]).await;
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
    let mut client = client::init().unwrap();

    let mut last_checkins = client::get(&mut client).unwrap();
    loop {
        let curr_checkins = client::get(&mut client).unwrap();

        let new_checkins: Vec<_> = curr_checkins.difference(&last_checkins).collect();
        if !new_checkins.is_empty() {
            for id in new_checkins {
                log::info!("new checkin: {:?}", id);
                let user_rgb = user_colors(*id);
                spin_leds(&user_rgb).await;
            }
        }

        last_checkins = curr_checkins;
        Timer::after(Duration::from_millis(1000)).await;
    }
}

// compile time env var - wifi psk
static PSK: &'static str = env!("PSK");

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
    let _wifi = wifi("Recurse Center", PSK, peripherals.modem, sysloop).unwrap();

    spawner.spawn(ambient_color_task()).unwrap();
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(checkin_task()).unwrap();

    log::info!("Main loop running...");
    loop {
        Timer::after(Duration::from_millis(1000)).await;
    }
}
