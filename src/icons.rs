//! Small embedded SVGs keep icons consistent across desktop themes.
use gtk::{gdk_pixbuf, prelude::*};
use std::{cell::RefCell, collections::HashMap};

pub const LIGHT: &str = "#c4d0e2";
pub const MUTED: &str = "#8291a8";
pub const ACCENT: &str = "#63dfc3";

fn svg(name: &str, color: &str) -> String {
    let paths = match name {
        "shield" => {
            "<path d='M12 3 20 6v6c0 4.3-3.1 7.3-8 9-4.9-1.7-8-4.7-8-9V6Z'/><path d='m8.5 12 2.3 2.3 4.7-4.8'/>"
        }
        "grid" => {
            "<rect x='3.5' y='3.5' width='6' height='6' rx='1.5'/><rect x='14.5' y='3.5' width='6' height='6' rx='1.5'/><rect x='3.5' y='14.5' width='6' height='6' rx='1.5'/><rect x='14.5' y='14.5' width='6' height='6' rx='1.5'/>"
        }
        "activity" => "<path d='M3 12h4l3-7 4 14 3-7h4'/>",
        "import" => "<path d='M12 3v12m-4-4 4 4 4-4M4 16v4h16v-4'/>",
        "export" => "<path d='M12 15V3m-4 4 4-4 4 4M4 16v4h16v-4'/>",
        "plus" => "<path d='M12 5v14M5 12h14'/>",
        "search" => "<circle cx='10.5' cy='10.5' r='6.5'/><path d='m16 16 4.5 4.5'/>",
        "refresh" => {
            "<path d='M20 10a8 8 0 0 0-13.7-4L3 9m0-5v5h5M4 14a8 8 0 0 0 13.7 4l3.3-3m0 5v-5h-5'/>"
        }
        "settings" => {
            "<path d='M4 7h16M4 17h16'/><circle cx='9' cy='7' r='3' fill='#131e30'/><circle cx='15' cy='17' r='3' fill='#131e30'/>"
        }
        "power" => "<path d='M12 3v9M6.3 6.3a8 8 0 1 0 11.4 0'/>",
        "arrow" => "<path d='M4 12h16m-6-6 6 6-6 6'/>",
        "copy" => "<rect x='8' y='8' width='12' height='13' rx='2'/><path d='M16 8V3H3v13h5'/>",
        "trash" => "<path d='M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7'/>",
        "more" => {
            "<circle cx='5' cy='12' r='1' fill='COLOR'/><circle cx='12' cy='12' r='1' fill='COLOR'/><circle cx='19' cy='12' r='1' fill='COLOR'/>"
        }
        "check" => "<path d='m5 12 4 4L19 6'/>",
        "minimize" => "<path d='M5 16h14'/>",
        "maximize" => "<rect x='5' y='5' width='14' height='14' rx='1'/>",
        "restore" => "<path d='M8 8V4h12v12h-4'/><rect x='4' y='8' width='12' height='12' rx='1'/>",
        "close" => "<path d='m6 6 12 12M6 18 18 6'/>",
        "clock" => "<circle cx='12' cy='12' r='9'/><path d='M12 7v5l3 2'/>",
        "folder" => "<path d='M3 7V4h7l2 3h9v13H3Z'/>",
        "info" => "<circle cx='12' cy='12' r='9'/><path d='M12 11v6M12 7v.2'/>",
        "wireguard" => "<path d='m14 2-9 12h6l-1 8 9-12h-6Z'/>",
        "network" => {
            "<rect x='8' y='3' width='8' height='5' rx='1'/><rect x='2' y='16' width='7' height='5' rx='1'/><rect x='15' y='16' width='7' height='5' rx='1'/><path d='M12 8v4M5.5 16v-4h13v4'/>"
        }
        "logout" => "<path d='M9 4H4v16h5m0-8h12m-4-4 4 4-4 4'/>",
        "chevron" => "<path d='m9 5 7 7-7 7'/>",
        _ => "<circle cx='12' cy='12' r='9'/>",
    };
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='{color}' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'>{}</svg>",
        paths.replace("COLOR", color)
    )
}

fn decode(bytes: &[u8], size: i32) -> gdk_pixbuf::Pixbuf {
    let loader = gdk_pixbuf::PixbufLoader::with_type("svg").expect("SVG loader");
    loader.set_size(size, size);
    loader.write(bytes).expect("embedded SVG");
    loader.close().expect("valid SVG");
    loader.pixbuf().expect("decoded SVG")
}

thread_local! {
    static CACHE: RefCell<HashMap<String, gdk_pixbuf::Pixbuf>> = RefCell::new(HashMap::new());
}

pub fn image(name: &str, color: &str, size: i32) -> gtk::Image {
    let key = format!("{name}/{color}/{size}");
    let pixbuf = CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| decode(svg(name, color).as_bytes(), size * 2))
            .clone()
    });
    let image = gtk::Image::from_pixbuf(Some(&pixbuf));
    image.set_pixel_size(size);
    image
}

pub fn brand(size: i32) -> gtk::Image {
    let pixbuf = decode(include_bytes!("../packaging/vpn-manager.svg"), size * 2);
    let image = gtk::Image::from_pixbuf(Some(&pixbuf));
    image.set_pixel_size(size);
    image
}

pub fn tray(active: bool) -> Vec<ksni::Icon> {
    let source: &[u8] = if active {
        include_bytes!("../packaging/vpn-manager-active.svg")
    } else {
        include_bytes!("../packaging/vpn-manager-tray.svg")
    };
    let p = decode(source, 48);
    let bytes = p.read_pixel_bytes();
    let stride = p.rowstride() as usize;
    let channels = p.n_channels() as usize;
    let mut argb = Vec::with_capacity(48 * 48 * 4);
    for y in 0..48 {
        for x in 0..48 {
            let i = y * stride + x * channels;
            argb.extend_from_slice(&[
                if channels == 4 { bytes[i + 3] } else { 255 },
                bytes[i],
                bytes[i + 1],
                bytes[i + 2],
            ]);
        }
    }
    vec![ksni::Icon {
        width: 48,
        height: 48,
        data: argb,
    }]
}

/// Explicit window icon for compositors that cache desktop theme lookups.
pub fn application_texture() -> gtk::gdk::Texture {
    gtk::gdk::Texture::for_pixbuf(&decode(include_bytes!("../packaging/vpn-manager.svg"), 128))
}
