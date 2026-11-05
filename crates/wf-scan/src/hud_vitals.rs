//! The local player's health, shields, overguard and energy, as the HUD shows them.
//!
//! The HUD keeps plain copies of what it draws. Its energy readout eases from the value it
//! showed last to the current one, and keeps both, with the maximum, in the HUD itself: the
//! code drawing `AbilityPanel.EnergyAmount` loads the eased-from value, the current one and
//! the maximum, in that order. Its health panel, hung off the HUD where the HUD wires it to
//! `HealthAndShield.PlayerHealth`, caches the numbers it last printed: its update adds the
//! shields to the health to tell whether there is anything to print, and stores the health,
//! loads the overguard and stores the shields one after the other. The maxima its bars are
//! split by are stored together, health, shields, how far over that shields can go, then
//! overguard, by the first call of that update that stores a run of them beside the numbers.
//! Nothing here is a fixed offset; it is all read out of that code.

use wf_mem::{MemoryReader, read_u32_le};

use crate::swig::Image;

const ENERGY_TEXT: &str = "AbilityPanel.EnergyAmount";
/// The energy bar's body, which the readout colours from a field of the HUD just after naming.
const ENERGY_BAR_TEXT: &str = "AbilityPanel.EnergyBar.Mover";
/// How far past naming the bar the readout loads its colour.
const COLOUR_SPAN: usize = 0x20;
/// The largest `0xRRGGBB`.
const MAX_COLOUR: f32 = 16_777_215.0;
const PANEL_TEXT: &str = "HealthAndShield.PlayerHealth";
/// How far past the panel's wiring the HUD calls its update.
const WIRING_SPAN: usize = 0x1000;
/// `mov rcx, [rcx]; call`: the HUD reading the panel out of its handle and updating it.
const CALL_UPDATE: [u8; 4] = [0x48, 0x8B, 0x09, CALL];
const UPDATE_GAP: usize = 16;
/// How much of the panel's update, and of each call it makes, to read.
const UPDATE_SPAN: usize = 0x8000;
const CALLEE_SPAN: usize = 0x400;
/// The maxima are stored in a run of this many, close after the numbers.
const MAXIMA_RUN: usize = 4;
const MAXIMA_NEAR: u32 = 0x40;
/// The energy readout's floats sit together.
const FIELDS_NEAR: u32 = 0x10;
const CALL: u8 = 0xE8;
const MOV_LOAD: u8 = 0x8B;
const MOV_STORE: u8 = 0x89;
const ADD_LOAD: u8 = 0x03;
const REX_W: u8 = 0x08;
const RCX: u8 = 1;
/// Larger than any health, shields or overguard the game gives.
const MAX_VALUE: i32 = 10_000_000;

/// What the HUD shows of the local player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vitals {
    pub health: u32,
    pub max_health: u32,
    pub shield: u32,
    pub max_shield: u32,
    /// How far over their maximum shields can go.
    pub max_overshield: u32,
    pub overguard: u32,
    pub max_overguard: u32,
    pub energy: f32,
    pub max_energy: f32,
    /// The colour the HUD draws the energy bar in, `0xRRGGBB`: the warframe's energy colour.
    pub energy_colour: Option<u32>,
}

/// Where the HUD keeps what it shows of the local player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudVitals {
    /// Fields of the HUD.
    energy: u64,
    max_energy: u64,
    energy_colour: Option<u64>,
    /// The HUD's field holding a handle to the health panel.
    panel: u64,
    /// Fields of the panel.
    health: u64,
    shield: u64,
    overguard: u64,
    max_health: u64,
    max_shield: u64,
    max_overshield: u64,
    max_overguard: u64,
}

impl HudVitals {
    /// `None` when the game's code no longer matches what this expects.
    pub fn locate(image: &Image) -> Option<Self> {
        let (energy, max_energy) = energy_fields(image)?;
        let (panel, calls) = panel_calls(image)?;
        let numbers = calls
            .into_iter()
            .find_map(|update| panel_numbers(image, update))?;
        Some(Self {
            energy: u64::from(energy),
            max_energy: u64::from(max_energy),
            energy_colour: energy_colour(image).map(u64::from),
            panel: u64::from(panel),
            health: u64::from(numbers.health),
            shield: u64::from(numbers.shield),
            overguard: u64::from(numbers.overguard),
            max_health: u64::from(numbers.max_health),
            max_shield: u64::from(numbers.max_shield),
            max_overshield: u64::from(numbers.max_overshield),
            max_overguard: u64::from(numbers.max_overguard),
        })
    }

    #[cfg(test)]
    pub(crate) fn at(energy: u64, panel: u64, health: u64) -> Self {
        Self {
            energy,
            max_energy: energy + 8,
            energy_colour: Some(energy + 0x10),
            panel,
            health,
            shield: health + 4,
            overguard: health + 0x14,
            max_health: health + 8,
            max_shield: health + 0xc,
            max_overshield: health + 0x10,
            max_overguard: health + 0x18,
        }
    }

    /// What the HUD at `hud` shows, or `None` while its health panel is not set up.
    pub fn read<R: MemoryReader + ?Sized>(&self, reader: &R, hud: u64) -> Option<Vitals> {
        let float = |at| {
            let bits = read_u32_le(&reader.read_vec(at, 4).ok()?, 0)?;
            Some(f32::from_bits(bits)).filter(|value| value.is_finite() && *value >= 0.0)
        };
        let energy = float(hud + self.energy)?;
        let max_energy = float(hud + self.max_energy)?;
        // The HUD hands the bar its colour as a float holding `0xRRGGBB`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let energy_colour = self
            .energy_colour
            .and_then(|field| float(hud + field))
            .filter(|colour| *colour <= MAX_COLOUR && colour.fract() == 0.0)
            .map(|colour| colour as u32);
        let panel = reader
            .read_u64(reader.read_u64(hud + self.panel).ok()?)
            .ok()?;
        let value = |field| {
            let bits = read_u32_le(&reader.read_vec(panel + field, 4).ok()?, 0)?;
            u32::try_from(bits.cast_signed().clamp(0, MAX_VALUE))
                .ok()
                .filter(|_| bits.cast_signed() < MAX_VALUE)
        };
        let vitals = Vitals {
            health: value(self.health)?,
            max_health: value(self.max_health)?,
            shield: value(self.shield)?,
            max_shield: value(self.max_shield)?,
            max_overshield: value(self.max_overshield)?,
            overguard: value(self.overguard)?,
            max_overguard: value(self.max_overguard)?,
            energy,
            max_energy,
            energy_colour,
        };
        (vitals.max_health > 0).then_some(vitals)
    }
}

/// The HUD's current and maximum energy: the second and third floats the energy readout
/// loads from the HUD, beside the first.
fn energy_fields(image: &Image) -> Option<(u32, u32)> {
    let text = image.text(ENERGY_TEXT)?;
    image.references(text).into_iter().find_map(|reference| {
        let start = image.function_start(reference)?;
        let code = image.code(start, usize::try_from(reference - start).ok()?)?;
        let [from, current, max, ..] = float_loads(code)[..] else {
            return None;
        };
        let fields = [from, current, max];
        let spread = fields.iter().max()? - fields.iter().min()?;
        (spread <= FIELDS_NEAR && from != current && current != max && from != max)
            .then_some((current, max))
    })
}

/// The HUD's field holding the health panel, loaded just before the panel is wired to its
/// clip, and the panel methods the HUD calls with it soon after, its update among them.
fn panel_calls(image: &Image) -> Option<(u32, Vec<u64>)> {
    let text = image.text(PANEL_TEXT)?;
    image.references(text).into_iter().find_map(|reference| {
        // `lea reg, [rip+text]` is 3 bytes before its displacement.
        let lea = reference.checked_sub(3)?;
        let before = image.code(lea.checked_sub(16)?, 16)?;
        let panel = (0..before.len()).find_map(|at| {
            let load = access(before, at)?;
            (at + load.len == before.len() && load.op == MOV_LOAD && load.rex & REX_W != 0)
                .then_some(load.disp)
        })?;
        let code = image.code(reference, WIRING_SPAN)?;
        let calls: Vec<u64> = (0..code.len())
            .filter_map(|at| {
                let load = access(code, at)?;
                if load.op != MOV_LOAD || load.reg != RCX || load.disp != panel {
                    return None;
                }
                let after = at + load.len;
                let gap = code.get(after..(after + UPDATE_GAP).min(code.len()))?;
                let call = after + memchr::memmem::find(gap, &CALL_UPDATE)? + 3;
                call_target(code, call, reference)
            })
            .collect();
        (!calls.is_empty()).then_some((panel, calls))
    })
}

/// Where the panel keeps its numbers, if `update` is the method that caches them.
fn panel_numbers(image: &Image, update: u64) -> Option<PanelNumbers> {
    let code = image.code(update, UPDATE_SPAN)?;
    let (health, shield) = health_and_shield(code)?;
    let overguard = overguard(code, health, shield)?;
    let [max_health, max_shield, max_overshield, max_overguard] = calls(code, update)
        .filter_map(|callee| image.code(callee, CALLEE_SPAN))
        .find_map(|callee| maxima(callee, health))?;
    Some(PanelNumbers {
        health,
        shield,
        overguard,
        max_health,
        max_shield,
        max_overshield,
        max_overguard,
    })
}

struct PanelNumbers {
    health: u32,
    shield: u32,
    overguard: u32,
    max_health: u32,
    max_shield: u32,
    max_overshield: u32,
    max_overguard: u32,
}

/// The panel's health and shield fields: `mov eax, [panel+shield]; add eax, [panel+health]`.
fn health_and_shield(code: &[u8]) -> Option<(u32, u32)> {
    (0..code.len()).find_map(|at| {
        let load = access(code, at)?;
        let add = access(code, at + load.len)?;
        (load.op == MOV_LOAD
            && add.op == ADD_LOAD
            && load.rex & REX_W == 0
            && add.rex & REX_W == 0
            && load.reg == add.reg
            && load.base == add.base
            && load.disp != add.disp)
            .then_some((add.disp, load.disp))
    })
}

/// The panel's overguard field, loaded between storing the health and the shields.
fn overguard(code: &[u8], health: u32, shield: u32) -> Option<u32> {
    (0..code.len()).find_map(|at| {
        let first = access(code, at)?;
        let load = access(code, at + first.len)?;
        let last = access(code, at + first.len + load.len)?;
        (first.op == MOV_STORE
            && first.disp == health
            && load.op == MOV_LOAD
            && last.op == MOV_STORE
            && last.disp == shield
            && first.base == load.base
            && load.base == last.base)
            .then_some(load.disp)
    })
}

/// The maxima a function stores in a run beside the numbers: health, shields, how far over
/// that shields can go, and overguard.
fn maxima(code: &[u8], health: u32) -> Option<[u32; MAXIMA_RUN]> {
    (0..code.len()).find_map(|at| {
        let mut stores = Vec::with_capacity(MAXIMA_RUN);
        let mut next = at;
        while stores.len() < MAXIMA_RUN {
            let store = access(code, next)?;
            let near = store.disp > health && store.disp - health <= MAXIMA_NEAR;
            if store.op != MOV_STORE
                || !near
                || stores
                    .first()
                    .is_some_and(|first: &Access| first.base != store.base)
            {
                return None;
            }
            next += store.len;
            stores.push(store);
        }
        Some([
            stores[0].disp,
            stores[1].disp,
            stores[2].disp,
            stores[3].disp,
        ])
    })
}

/// Where each direct call in `code`, which starts at `start`, goes, in order.
fn calls(code: &[u8], start: u64) -> impl Iterator<Item = u64> + '_ {
    (0..code.len())
        .filter(|&at| code[at] == CALL)
        .filter_map(move |at| call_target(code, at, start))
}

fn call_target(code: &[u8], at: usize, start: u64) -> Option<u64> {
    let rel = i64::from(read_u32_le(code, at + 1)?.cast_signed());
    start.checked_add_signed(i64::try_from(at + 5).ok()? + rel)
}

/// The HUD's field holding the energy bar's colour: the first float the readout loads after
/// naming the bar's body.
fn energy_colour(image: &Image) -> Option<u32> {
    let text = image.text(ENERGY_BAR_TEXT)?;
    image.references(text).into_iter().find_map(|reference| {
        let code = image.code(reference, COLOUR_SPAN)?;
        (0..code.len()).find_map(|at| float_load(code.get(at..)?, None))
    })
}

/// `movss xmm, [reg+disp32]` displacements in `code`, in order, from `rcx` alone.
fn float_loads(code: &[u8]) -> Vec<u32> {
    (0..code.len())
        .filter_map(|at| float_load(code.get(at..)?, Some(RCX)))
        .collect()
}

/// The displacement of `movss xmm, [reg+disp32]` at the start of `code`, from `base` if given.
fn float_load(code: &[u8], base: Option<u8>) -> Option<u32> {
    let [0xF3, rest @ ..] = code else {
        return None;
    };
    let rest = match rest {
        [rex @ 0x40..=0x47, rest @ ..] if rex & 1 == 0 => rest,
        rest => rest,
    };
    let [0x0F, 0x10, modrm, ..] = *rest else {
        return None;
    };
    // mod 10 (disp32), no SIB
    let register = modrm & 7;
    (modrm >> 6 == 2 && register != 4 && base.is_none_or(|base| base == register))
        .then(|| read_u32_le(rest, 3))?
}

/// An instruction reading or writing memory at a register plus a 32-bit displacement:
/// `[rex] op modrm disp32`, mod 10, no SIB byte.
#[derive(Debug, Clone, Copy)]
struct Access {
    len: usize,
    rex: u8,
    op: u8,
    reg: u8,
    base: u8,
    disp: u32,
}

fn access(code: &[u8], at: usize) -> Option<Access> {
    let (rex, op_at) = match *code.get(at)? {
        rex @ 0x40..=0x4F => (rex, at + 1),
        _ => (0, at),
    };
    let op = *code.get(op_at)?;
    let modrm = *code.get(op_at + 1)?;
    if modrm >> 6 != 2 || modrm & 7 == 4 {
        return None;
    }
    Some(Access {
        len: op_at + 6 - at,
        rex,
        op,
        reg: (modrm >> 3 & 7) | (rex >> 2 & 1) << 3,
        base: (modrm & 7) | (rex & 1) << 3,
        disp: read_u32_le(code, op_at + 2)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeReader;

    const HEAP: u64 = 0x2_0000_0000;

    #[test]
    fn finds_the_energy_readouts_floats() {
        // movss xmm0, [rcx+0x504]; mov rbx, rcx; movss xmm6, [rcx+0x500]; movss xmm2, [rcx+0x508]
        let code = [
            0xF3, 0x0F, 0x10, 0x81, 0x04, 0x05, 0, 0, 0x48, 0x8B, 0xD9, 0xF3, 0x0F, 0x10, 0xB1,
            0x00, 0x05, 0, 0, 0xF3, 0x0F, 0x10, 0x91, 0x08, 0x05, 0, 0,
        ];
        assert_eq!(float_loads(&code), [0x504, 0x500, 0x508]);
    }

    #[test]
    fn finds_the_panels_numbers() {
        // mov eax, [r14+0x2dc]; add eax, [r14+0x2d8]
        let sum = [
            0x41, 0x8B, 0x86, 0xDC, 0x02, 0, 0, 0x41, 0x03, 0x86, 0xD8, 0x02, 0, 0,
        ];
        assert_eq!(health_and_shield(&sum), Some((0x2d8, 0x2dc)));
        // mov [r14+0x2d8], eax; mov eax, [r14+0x2ec]; mov [r14+0x2dc], edi
        let cache = [
            0x41, 0x89, 0x86, 0xD8, 0x02, 0, 0, 0x41, 0x8B, 0x86, 0xEC, 0x02, 0, 0, 0x41, 0x89,
            0xBE, 0xDC, 0x02, 0, 0,
        ];
        assert_eq!(overguard(&cache, 0x2d8, 0x2dc), Some(0x2ec));
        // mov [rbx+0x2e0], r15d; mov [rbx+0x2e4], esi; mov [rbx+0x2e8], ebp;
        // mov [rbx+0x2f0], r14d
        let stores = [
            0x44, 0x89, 0xBB, 0xE0, 0x02, 0, 0, 0x89, 0xB3, 0xE4, 0x02, 0, 0, 0x89, 0xAB, 0xE8,
            0x02, 0, 0, 0x44, 0x89, 0xB3, 0xF0, 0x02, 0, 0,
        ];
        assert_eq!(maxima(&stores, 0x2d8), Some([0x2e0, 0x2e4, 0x2e8, 0x2f0]));
        assert_eq!(maxima(&stores[7..], 0x2d8), None);
    }

    #[test]
    fn reads_what_the_hud_shows() {
        let mut heap = vec![0u8; 0x1000];
        let (hud, handle, panel) = (HEAP + 0x100, HEAP + 0x800, HEAP + 0x900);
        let mut put = |at: u64, bytes: &[u8]| {
            let at = usize::try_from(at - HEAP).expect("offset");
            heap[at..at + bytes.len()].copy_from_slice(bytes);
        };
        put(hud + 0x500, &628.5f32.to_le_bytes());
        put(hud + 0x508, &700.0f32.to_le_bytes());
        put(hud + 0x510, &(0xa8_eddb as f32).to_le_bytes());
        put(hud + 0x6b0, &handle.to_le_bytes());
        put(handle, &panel.to_le_bytes());
        for (field, value) in [
            (0x2d8, 1981u32),
            (0x2dc, 1649),
            (0x2e0, 2000),
            (0x2e4, 1700),
            (0x2e8, 1200),
        ] {
            put(panel + field, &value.to_le_bytes());
        }
        let reader = FakeReader::new(vec![(HEAP, heap)]);
        let vitals = HudVitals::at(0x500, 0x6b0, 0x2d8);
        assert_eq!(
            vitals.read(&reader, hud),
            Some(Vitals {
                health: 1981,
                max_health: 2000,
                shield: 1649,
                max_shield: 1700,
                max_overshield: 1200,
                overguard: 0,
                max_overguard: 0,
                energy: 628.5,
                max_energy: 700.0,
                energy_colour: Some(0xa8_eddb),
            })
        );
        let unset = HudVitals::at(0x500, 0x6b0, 0x2e8);
        assert_eq!(unset.read(&reader, hud), None);
    }
}
