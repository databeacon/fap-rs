//! # Fabulous APRS Parser
//!
//! This is a Rust wrapper around Fabulous (or, perhaps, Finnish... or both) APRS Parser, aka 
//! [`libfap`](http://www.pakettiradio.net/libfap/). 
//!
//! To parse a packet:
//!
//! ```rust
//! extern crate aprs; 
//! extern crate fap;
//! use aprs::{Packet, Position, Degrees, Knots}; 
//!
//! let raw = "DISCOF>APT314,RAZOR*,WIDE1*,qAS,GERLCH:/022526h4046.40N/11912.12W-347/001/";
//! let parsed = fap::Packet::new(raw);
//! 
//! match parsed {
//!     Ok(packet) => {
//!         assert_eq!(packet.source(), "DISCOF");
//!         assert_eq!(packet.latitude(), Some(40.7733335));
//!         assert_eq!(packet.longitude(), Some(-119.202));
//!         assert_eq!(packet.course(), Some(Degrees(347.0)));
//!     },
//!     Err(_) => {
//!         panic!("Bad packet!")
//!     }
//! }
//! ```
//!
//! Parsed packet implements `aprs::Packet` trait, see [`aprs` crate documentation](https://docs.rs/aprs) 
//! for details on how to use the returned value. 
//! 
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

extern crate aprs;

mod bind {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
use bind::*;
use aprs::{Packet as AprsPacket, Position, Feet, Knots, KilometersPerHour,
    Meters, Degrees, Fahrenheits, Symbol};
use std::ffi::{CStr, CString, NulError};
use std::os::raw::{c_uint, c_short, c_char};
use std::sync::{Once, ONCE_INIT};
use std::borrow::Cow;
use std::vec::Vec;
use std::fmt;
use std::time::{SystemTime, Duration, UNIX_EPOCH};


#[derive(Debug)]
pub enum Error{
    NulInInputData(NulError),
    Other(String)
}
impl std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::NulInInputData(_) => "input data must not contain any nulls",
            Error::Other(msg) => msg.as_str(), 
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match self {
            Error::NulInInputData(ref err) => Some(err),
            Error::Other(_) => None, 
        }        
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NulInInputData(err) => write!(f, "input data must not contain any nulls: {}", err),
            Error::Other(msg) => write!(f, "{}", msg), 
        }        
    }
}

static INIT: Once = ONCE_INIT;

#[derive(Debug)]
pub struct Packet { 
    ptr: *mut fap_packet_t, 
}

impl Drop for Packet {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { fap_free(self.ptr); }                
        }
    }   
}

impl Packet {
    pub fn new<T: Into<Vec<u8>>>(data: T) -> Result<Packet, Error> {
        let data = data.into();
        let len = data.len();
        let data = CString::new(data).map_err(|e| Error::NulInInputData(e))?;
        unsafe {
            INIT.call_once(|| {
                fap_init();
            });
            let ptr = fap_parseaprs(data.as_ptr() as *const c_char, len as c_uint, 0 as c_short);
            if ptr.is_null() {
                return Err(Error::Other("libfap returned null value - allocation failure?".to_string()))
            }   
            let packet = Packet{ ptr }; 
            if !packet.fap().error_code.is_null() {
                let buf = &mut [0 as i8; 64];
                fap_explain_error(*packet.fap().error_code, buf.as_mut_ptr() as *mut c_char);
                let msg = CStr::from_ptr(buf.as_ptr() as *mut c_char);
                return Err(Error::Other(msg.to_string_lossy().into_owned()));
            }
            Ok(packet) 
        }       
    }

    fn fap(&self) -> fap_packet_t {
        debug_assert!(!self.ptr.is_null());
        unsafe { *self.ptr }
    }

    pub fn header(&self) -> Cow<str> {
        debug_assert!(!self.fap().header.is_null());
        unsafe{ CStr::from_ptr(self.fap().header) }.to_string_lossy()
    }

    pub fn body(&self) -> Cow<str>  {
        debug_assert!(!self.fap().body.is_null());
        unsafe{ CStr::from_ptr(self.fap().body) }.to_string_lossy()
    }
}

impl AprsPacket for Packet {
    fn source(&self) -> Cow<str> {
        debug_assert!(!self.fap().src_callsign.is_null());
        unsafe{ CStr::from_ptr(self.fap().src_callsign) }.to_string_lossy()
    }

    fn symbol(&self) -> Symbol {
        Symbol::from_table(self.fap().symbol_table as u8, self.fap().symbol_code as u8)
    }

    fn timestamp(&self) -> Option<SystemTime> {
        if self.fap().timestamp.is_null() {
            return None
        }
        let ts = unsafe{*self.fap().timestamp };
        Some(UNIX_EPOCH + Duration::from_secs(ts as u64))
    }

    fn destination(&self) -> Option<Cow<str>> {
        if self.fap().dst_callsign.is_null() {
            return None
        }
        Some(unsafe{ CStr::from_ptr(self.fap().dst_callsign) }.to_string_lossy())
    }

    fn comment(&self) -> Option<Cow<str>> {
        if self.fap().comment.is_null() {
            return None
        }
        Some(unsafe{ CStr::from_ptr(self.fap().comment) }.to_string_lossy())
    }

    fn latitude(&self) -> Option<f32> {
        if self.fap().latitude.is_null() {
            return None
        }
        Some(unsafe{*self.fap().latitude as f32})
    }

    fn longitude(&self) -> Option<f32> {
        if self.fap().longitude.is_null() {
            return None
        }
        Some(unsafe{*self.fap().longitude as f32})
    }

    fn precision(&self) -> Option<Feet> {
        if self.fap().pos_resolution.is_null() {
            return None
        }
        Some(Feet::from(Meters(unsafe{*self.fap().pos_resolution as f32})))
    }

    fn position(&self) -> Option<aprs::Position> {
        let lat = self.latitude();
        let lng = self.longitude();
        if lat.is_none() || lng.is_none() {
            return None
        }
        match self.precision() {
            Some(p) => Some(Position::from_latlng_precise(lat.unwrap(), lng.unwrap(), p)),
            None => Some(Position::from_latlng(lat.unwrap(), lng.unwrap()))
        }
    }

    fn speed(&self) -> Option<Knots> {
        if self.fap().speed.is_null() {
            return None
        }
        Some(Knots::from(KilometersPerHour(unsafe{*self.fap().speed as f32})))
    }

    fn course(&self) -> Option<Degrees> {
        if self.fap().course.is_null() {
            return None
        }
        let mut v = unsafe{*self.fap().course };
        if v == 0 {
            return None // 0 means "unknown" in libfap
        }
        v = v % 360;
        Some(Degrees(v as f32))
    }

    fn altitude(&self) -> Option<Feet> {
        if self.fap().altitude.is_null() {
            return None
        }
        Some(Feet::from(Meters(unsafe{*self.fap().altitude as f32})))
    }

    fn temperature(&self) -> Option<Fahrenheits> {
        unimplemented!();
    }

    fn wind_direction(&self) -> Option<Degrees> {
        unimplemented!();
    }

    fn wind_speed(&self) -> Option<Knots> {
        unimplemented!();
    }
}

impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:9} {:19} @ {:?}", self.source(), format!("{:?}", self.symbol()), self.position())
    }
}
