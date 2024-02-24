use std::io::{Cursor, Read, Seek};
use std::sync::RwLock;

use exr::prelude::{ReadChannels, ReadLayers, ReadSpecificChannel, Vec2};

static IMAGE32F: RwLock<Option<Img<f32>>> = RwLock::new(None);

static EXR_ERR: RwLock<Option<String>> = RwLock::new(None);

static EXR_INPUT: RwLock<Option<Vec<u8>>> = RwLock::new(None);

struct Img<T> {
    data: Vec<T>,
    pos: Vec2<i32>,
    size: Vec2<usize>,
}

fn _exr_size() -> Result<Vec2<usize>, &'static str> {
    let guard = IMAGE32F.try_read().map_err(|_| "unable to read lock")?;
    let oimg: &Option<_> = &guard;
    let osz: Option<_> = oimg.as_ref().map(|i| i.size);
    Ok(osz.unwrap_or_default())
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn exr_ptr() -> *const u8 {
    match IMAGE32F.try_read().ok() {
        None => std::ptr::null(),
        Some(guard) => {
            let oi: &Option<_> = &guard;
            match oi {
                None => std::ptr::null(),
                Some(img) => img.data.as_ptr() as *const u8,
            }
        }
    }
}

/// Gets the width of an exr image.
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn exr_width() -> i32 {
    _exr_size()
        .ok()
        .map(|p| p.0)
        .and_then(|u| u.try_into().ok())
        .unwrap_or(-1)
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn exr_height() -> i32 {
    _exr_size()
        .ok()
        .map(|p| p.1)
        .and_then(|u| u.try_into().ok())
        .unwrap_or(-1)
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn exr_emsg_ptr() -> *const u8 {
    EXR_ERR
        .try_read()
        .ok()
        .and_then(|guard| {
            let o: &Option<_> = &guard;
            o.as_ref().map(|s| s.as_ptr())
        })
        .unwrap_or_else(std::ptr::null)
}

fn _exr_msg_sz() -> Result<usize, &'static str> {
    let guard = EXR_ERR.try_read().map_err(|_| "unable to read lock emsg")?;
    let o: &Option<_> = &guard;
    Ok(o.as_ref().map(|s| s.len()).unwrap_or(0))
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn exr_msg_sz() -> i32 {
    _exr_msg_sz()
        .ok()
        .and_then(|u| u.try_into().ok())
        .unwrap_or(-1)
}

fn _exr_set_err(emsg: &str) -> Result<(), &'static str> {
    let mut guard = EXR_ERR
        .try_write()
        .map_err(|_| "unable to write lock emsg")?;
    let mo: &mut Option<_> = &mut guard;
    let mut s: String = mo.take().unwrap_or_default();
    s.clear();
    s.push_str(emsg);
    mo.replace(s);
    Ok(())
}

fn exr_set_err(e: exr::error::Error) -> Result<(), &'static str> {
    match e {
        exr::error::Error::Aborted => _exr_set_err("Aborted"),
        exr::error::Error::NotSupported(msg) => _exr_set_err(&msg),
        exr::error::Error::Invalid(msg) => _exr_set_err(&msg),
        exr::error::Error::Io(_) => _exr_set_err("I/O Error"),
    }
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn exr_input_ptr() -> *const u8 {
    EXR_INPUT
        .try_read()
        .ok()
        .and_then(|guard| {
            let ov: &Option<_> = &guard;
            ov.as_ref().map(|v| v.as_ptr())
        })
        .unwrap_or_else(std::ptr::null)
}

fn chan2arr32f<R>(
    chan_name: &str,
    rdr: R,
    out: &mut Vec<f32>,
    opos: &mut Vec2<i32>,
    sz: &mut Vec2<usize>,
) where
    R: Read + Seek,
{
    let img: Result<_, _> = exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .specific_channels()
        .required(chan_name)
        .collect_pixels(
            |res, (_chan,)| {
                let w: usize = res.width();
                let h: usize = res.height();
                let sz: usize = w * h;
                let v: Vec<f32> = Vec::with_capacity(sz);
                v
            },
            |v, _pos, (val,): (f32,)| {
                v.push(val);
            },
        )
        .all_layers()
        .all_attributes()
        .from_buffered(rdr);
    match img {
        Ok(_) => {}
        Err(e) => {
            exr_set_err(e).ok();
            return;
        }
    }
    let img = img.ok();
    let bounds: Option<_> = img.as_ref().map(|i| i.attributes.display_window);
    let pos: Option<_> = bounds.map(|i| i.position);
    let size: Option<_> = bounds.map(|i| i.size);
    let layer: Option<_> = img.and_then(|i| i.layer_data.into_iter().next());
    let chan: Option<_> = layer.map(|l| l.channel_data);
    let pixels: Option<Vec<f32>> = chan.map(|c| c.pixels);
    let image = pixels.and_then(|v| {
        pos.and_then(|p| {
            size.map(|s| Img {
                data: v,
                pos: p,
                size: s,
            })
        })
    });
    match image {
        None => {}
        Some(mut i) => {
            out.append(&mut i.data);
            *opos = i.pos;
            *sz = i.size;
        }
    }
}

fn chan2img32f<R>(chan_name: &str, rdr: R, cap: usize) -> Result<usize, &'static str>
where
    R: Read + Seek,
{
    let mut guard = IMAGE32F
        .try_write()
        .map_err(|_| "unable to write lock image buf")?;
    let mi: &mut Option<_> = &mut guard;
    let mut i: Img<f32> = mi.take().unwrap_or_else(|| Img {
        data: Vec::with_capacity(cap),
        pos: Vec2::default(),
        size: Vec2::default(),
    });
    chan2arr32f(chan_name, rdr, &mut i.data, &mut i.pos, &mut i.size);
    let datsz: usize = i.data.len();
    mi.replace(i);
    Ok(datsz)
}

fn _y2img32f(data: &[u8], cap: usize) -> Result<usize, &'static str> {
    let rdr = Cursor::new(data);
    let chan_name: &str = "Y";
    chan2img32f(chan_name, rdr, cap)
}

fn _exr_reset(sz: usize) -> Result<usize, &'static str> {
    let mut guard = EXR_INPUT
        .try_write()
        .map_err(|_| "unable to write lock exr buf")?;
    let mo: &mut Option<_> = &mut guard;
    let mut v: Vec<u8> = mo.take().unwrap_or_else(|| Vec::with_capacity(sz));
    v.resize(sz, 0);
    let sz: usize = v.len();
    mo.replace(v);
    Ok(sz)
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn exr_reset(sz: i32) -> i32 {
    sz.try_into()
        .ok()
        .and_then(|u: usize| _exr_reset(u).ok())
        .and_then(|u: usize| u.try_into().ok())
        .unwrap_or(-1)
}

fn y2img32f(cap: usize) -> Result<usize, &'static str> {
    let guard = EXR_INPUT.try_read().map_err(|_| "unable to read lock")?;
    let o: &Option<_> = &guard;
    let s: &[u8] = o.as_deref().unwrap_or_default();
    _y2img32f(s, cap)
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn y2image32f(cap: i32) -> i32 {
    cap.try_into()
        .ok()
        .and_then(|u: usize| y2img32f(u).ok())
        .and_then(|u: usize| u.try_into().ok())
        .unwrap_or(-1)
}
