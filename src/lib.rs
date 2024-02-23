#[cfg(feature = "exr_jv")]
pub mod exr;

use std::sync::RwLock;

static INPUT: RwLock<Option<Vec<u8>>> = RwLock::new(None);

fn _reset(sz: usize) -> Result<usize, &'static str> {
    let mut guard = INPUT
        .try_write()
        .map_err(|_| "unable to lock input bytes")?;
    let mo: &mut Option<_> = &mut guard;
    let mut v: Vec<_> = mo.take().unwrap_or_else(|| Vec::with_capacity(sz));
    v.clear();
    v.resize(sz, 0);
    let sz: usize = v.len();
    mo.replace(v);
    Ok(sz)
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn reset(sz: i32) -> i32 {
    sz.try_into()
        .ok()
        .and_then(|u: usize| _reset(u).ok())
        .and_then(|cap: usize| cap.try_into().ok())
        .unwrap_or(-1)
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn get_ptr() -> *const u8 {
    match INPUT.try_read() {
        Err(_) => std::ptr::null(),
        Ok(guard) => {
            let o: &Option<_> = &guard;
            match o {
                None => std::ptr::null(),
                Some(v) => v.as_ptr(),
            }
        }
    }
}

fn slice_fold<S, T, R, C>(s: &[u8], init: S, chunk2t: C, reducer: R) -> S
where
    T: Sized,
    C: Fn(&[u8]) -> T,
    R: Fn(S, T) -> S,
{
    let sz: usize = core::mem::size_of::<T>();
    let chunks = s.chunks(sz);
    let mapd = chunks.map(chunk2t);
    mapd.fold(init, reducer)
}

fn fold32f<R>(init: f32, reducer: R) -> f32
where
    R: Fn(f32, f32) -> f32,
{
    match INPUT.try_read() {
        Err(_) => init,
        Ok(guard) => {
            let o: &Option<_> = &guard;
            match o {
                None => init,
                Some(v) => {
                    let s: &[u8] = v;
                    slice_fold(
                        s,
                        init,
                        |chunk: &[u8]| {
                            let a: [u8; 4] = chunk.try_into().ok().unwrap_or_default();
                            f32::from_be_bytes(a)
                        },
                        reducer,
                    )
                }
            }
        }
    }
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn sum32f() -> f32 {
    fold32f(0.0, |state: f32, next: f32| state + next)
}
