#![no_main]
#![no_std]

mod timer;

use core::panic::PanicInfo;
use rtic::app;
use rtt_target::rprintln;

#[app(device = pac, peripherals = true, dispatchers = [PDM, QDEC])]
mod app {
    use crate::timer::Timer;
    use embedded_graphics::image::{Image, ImageRaw, ImageRawLE};
    use embedded_graphics::pixelcolor::Rgb565;
    use embedded_graphics::prelude::*;
    use hal::clocks::{Clocks, LfOscConfiguration};
    use hal::delay::Delay;
    use hal::gpio::{p0, p1, Level, Output, PushPull};
    use hal::spim;
    use nrf52840_hal as hal;
    use nrf52840_pac as pac;
    use num_traits::float::Float;
    use rtt_target::{rprintln, rtt_init_print};
    use st7735_lcd;
    use st7735_lcd::Orientation;

    const SCREEN_WIDTH: usize = 64;
    const SCREEN_HEIGHT: usize = 64;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        timer1: pac::TIMER1,
        disp: st7735_lcd::ST7735<
            spim::Spim<pac::SPIM1>,
            p1::P1_08<Output<PushPull>>,
            p0::P0_07<Output<PushPull>>,
        >,
        bytes: [u8; SCREEN_HEIGHT * SCREEN_WIDTH * 2],
        t: u32,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Configure to use external clocks, and start them
        Clocks::new(ctx.device.CLOCK)
            .enable_ext_hfosc()
            .set_lfclk_src_external(LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        ctx.core.DCB.enable_trace();
        ctx.core.DWT.enable_cycle_counter();
        rtt_init_print!();
        rprintln!("RTT initialized");

        let interval = 1_000;

        let mut timer1 = ctx.device.TIMER1;
        timer1.init();
        timer1.fire_at(1, interval);

        rprintln!("Timers initialized");
        // Set up GPIO ports
        let p0 = p0::Parts::new(ctx.device.P0);
        let p1 = p1::Parts::new(ctx.device.P1);

        let mut delay = Delay::new(ctx.core.SYST);

        let spiclk = p0.p0_14.into_push_pull_output(Level::Low).degrade();
        let spimosi = p0.p0_13.into_push_pull_output(Level::Low).degrade();
        let pins = spim::Pins {
            sck: spiclk,
            miso: None,
            mosi: Some(spimosi),
        };
        rprintln!("SPIM initialized");
        let spim = spim::Spim::new(ctx.device.SPIM1, pins, spim::Frequency::M8, spim::MODE_0, 0);
        let dc = p1.p1_08.into_push_pull_output(Level::Low);
        let rst = p0.p0_07.into_push_pull_output(Level::Low);
        let mut disp = st7735_lcd::ST7735::new(
            spim,
            dc,
            rst,
            true,
            false,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
        );
        disp.init(&mut delay).unwrap();
        disp.set_orientation(&Orientation::LandscapeSwapped)
            .unwrap();
        disp.set_offset(0, 0);
        disp.clear(Rgb565::BLACK).unwrap();
        rprintln!("Display initialized");

        // draw ferris
        // let bytes = *include_bytes!("ferris.raw");
        // rprintln!("Displaying image");

        // We're all set up, hand off control back to RTIC
        let shared = Shared {};

        let local = Local {
            timer1,
            disp,
            bytes: [0; SCREEN_HEIGHT * SCREEN_WIDTH * 2],
            t: 0,
        };

        (shared, local, init::Monotonics())
    }

    #[task(binds = TIMER1, local = [
        timer1,
        disp,
        bytes,
        t,
    ])]
    fn timer1(ctx: timer1::Context) {
        let timer = ctx.local.timer1;
        let disp = ctx.local.disp;
        let bytes = ctx.local.bytes;
        let t = ctx.local.t;

        timer.ack_compare_event(1);

        for i in 0..SCREEN_HEIGHT {
            for j in 0..SCREEN_WIDTH {
                let x = i as f32 / SCREEN_HEIGHT as f32;
                let y = j as f32 / SCREEN_WIDTH as f32;
                let r = 0.5 + 0.5 * (*t as f32 + x + 0.0).cos();
                let g = 0.5 + 0.5 * (*t as f32 + y + 2.0).cos();
                let b = 0.5 + 0.5 * (*t as f32 + x + 4.0).cos();

                let r5 = (r * 31.0) as u16;
                let g6 = (g * 63.0) as u16;
                let b5 = (b * 31.0) as u16;

                let x = (b5 << 11) + (g6 << 5) + r5;
                let [hi, low] = x.to_le_bytes();

                bytes[(i * SCREEN_HEIGHT + j) * 2 + 0] = hi;
                bytes[(i * SCREEN_HEIGHT + j) * 2 + 1] = low;
            }
        }

        let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(bytes, SCREEN_WIDTH as u32);
        let image = Image::new(&image_raw, Point::new(0, 0));

        disp.set_offset(0, 0);
        image.draw(disp).unwrap();
        disp.set_offset(67, 0);
        image.draw(disp).unwrap();
        disp.set_offset(0, 66);
        image.draw(disp).unwrap();
        disp.set_offset(67, 66);
        image.draw(disp).unwrap();

        *t = t.wrapping_add(1);

        let _ = timer.fire_at(1, 1000);
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }
}

#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    rprintln!("{}", info);
    loop {}
}
