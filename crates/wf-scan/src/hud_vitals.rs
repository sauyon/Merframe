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
//! The panel keeps the colours it draws in too, as `0xRRGGBB`: one of its methods loads the
//! health, overshield and shield colours one after the other just after naming the bars'
//! `DotPreviewFillMask`, and the overguard colour just before printing the overguard as
//! `%02X%02X%02X`. Nothing here is a fixed offset; it is all read out of that code.

use wf_mem::{MemoryReader, read_u32_le};

use crate::swig::Image;

const ENERGY_TEXT: &str = "AbilityPanel.EnergyAmount";
/// The energy bar's body, which the readout colours from a field of the HUD just after naming.
const ENERGY_BAR_TEXT: &str = "AbilityPanel.EnergyBar.Mover";
/// How far past naming the bar the readout loads its colour.
const COLOUR_SPAN: usize = 0x20;
/// The largest `0xRRGGBB`, and as a float.
const MAX_RGB: u32 = 0xFF_FFFF;
const MAX_COLOUR: f32 = 16_777_215.0;
const PANEL_TEXT: &str = "HealthAndShield.PlayerHealth";
/// The bars' mask, which the panel names just before loading their colours.
const FILL_MASK_TEXT: &str = "DotPreviewFillMask";
/// How far past naming the mask the panel loads the bars' colours.
const BAR_COLOURS_SPAN: usize = 0x100;
/// The format the panel prints the overguard's colour in, just after loading it.
const HEX_COLOUR_TEXT: &str = "%02X%02X%02X";
/// How far before printing the overguard's colour the panel loads it.
const OVERGUARD_COLOUR_SPAN: usize = 0x40;
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
const LEA: u8 = 0x8D;
/// `rex lea reg, [rip+disp32]`.
const LEA_LEN: usize = 7;
/// `ModRM` mod 00, r/m 101: `[rip+disp32]`.
const RIP_RELATIVE: u8 = 0x05;
const REX_W: u8 = 0x08;
const RCX: u8 = 1;
/// Larger than any health, shields or overguard the game gives.
const MAX_VALUE: u32 = 10_000_000;

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
    /// The colours the HUD draws the other bars in, `0xRRGGBB`.
    pub health_colour: Option<u32>,
    pub shield_colour: Option<u32>,
    pub overshield_colour: Option<u32>,
    pub overguard_colour: Option<u32>,
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
    colours: Option<PanelColours<u64>>,
}

/// The panel's colour for each bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PanelColours<T> {
    health: T,
    shield: T,
    overshield: T,
    overguard: T,
}

impl HudVitals {
    /// `None` when the game's code no longer matches what this expects.
    pub fn locate(image: &Image) -> Option<Self> {
        let (energy, max_energy) = energy_fields(image)?;
        let (panel, calls) = panel_calls(image)?;
        let numbers = calls
            .iter()
            .find_map(|&update| panel_numbers(image, update))?;
        let colours = calls
            .iter()
            .find_map(|&method| panel_colours(image, method));
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
            colours: colours.map(|colours| PanelColours {
                health: u64::from(colours.health),
                shield: u64::from(colours.shield),
                overshield: u64::from(colours.overshield),
                overguard: u64::from(colours.overguard),
            }),
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
            colours: Some(PanelColours {
                health: health - 0xec,
                shield: health - 0xe8,
                overshield: health - 0xe4,
                overguard: health - 0xd0,
            }),
        }
    }

    /// What the HUD at `hud` shows, or `None` while its health panel is not set up.
    pub fn read<R: MemoryReader + ?Sized>(&self, reader: &R, hud: u64) -> Option<Vitals> {
        let float = |field: u64| {
            let bits = read_u32_le(&reader.read_vec(hud.checked_add(field)?, 4).ok()?, 0)?;
            Some(f32::from_bits(bits)).filter(|value| value.is_finite() && *value >= 0.0)
        };
        let energy = float(self.energy)?;
        let max_energy = float(self.max_energy)?;
        // The HUD hands the bar its colour as a float holding `0xRRGGBB`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let energy_colour = self
            .energy_colour
            .and_then(float)
            .filter(|colour| *colour <= MAX_COLOUR && colour.fract() == 0.0)
            .map(|colour| colour as u32);
        let panel = reader
            .read_u64(reader.read_u64(hud.checked_add(self.panel)?).ok()?)
            .ok()?;
        let value = |field: u64| {
            let bits = read_u32_le(&reader.read_vec(panel.checked_add(field)?, 4).ok()?, 0)?;
            // A negative number is shown as none.
            u32::try_from(bits.cast_signed().max(0))
                .ok()
                .filter(|&value| value < MAX_VALUE)
        };
        let colour = |field: Option<u64>| {
            let bits = read_u32_le(&reader.read_vec(panel.checked_add(field?)?, 4).ok()?, 0)?;
            Some(bits).filter(|&colour| colour <= MAX_RGB)
        };
        let colours = self.colours;
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
            health_colour: colour(colours.map(|colours| colours.health)),
            shield_colour: colour(colours.map(|colours| colours.shield)),
            overshield_colour: colour(colours.map(|colours| colours.overshield)),
            overguard_colour: colour(colours.map(|colours| colours.overguard)),
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
        .find_map(|callee| maxima(callee, health, &[shield, overguard]))?;
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

/// Where the panel keeps the colours it draws in, if `method` is the one that loads them.
fn panel_colours(image: &Image, method: u64) -> Option<PanelColours<u32>> {
    let code = image.code(method, UPDATE_SPAN)?;
    let [health, overshield, shield] = naming(image, FILL_MASK_TEXT, method, code)
        .into_iter()
        .find_map(|after| {
            let window = code.get(after..(after + BAR_COLOURS_SPAN).min(code.len()))?;
            let mut loads = colour_loads(window).into_iter();
            let first = loads.next()?;
            let mut rest = loads.filter(|load| load.base == first.base);
            let colours = [first, rest.next()?, rest.next()?];
            let distinct = colours[0].disp != colours[1].disp
                && colours[1].disp != colours[2].disp
                && colours[0].disp != colours[2].disp;
            distinct.then_some(colours)
        })?;
    let overguard = naming(image, HEX_COLOUR_TEXT, method, code)
        .into_iter()
        .find_map(|after| {
            let end = after.checked_sub(LEA_LEN)?;
            let window = code.get(end.saturating_sub(OVERGUARD_COLOUR_SPAN)..end)?;
            colour_loads(window)
                .into_iter()
                .rev()
                .find(|load| load.base == health.base)
        })?;
    Some(PanelColours {
        health: health.disp,
        shield: shield.disp,
        overshield: overshield.disp,
        overguard: overguard.disp,
    })
}

/// Where in `code`, the function at `start`, each `lea reg, [rip+disp32]` of a copy of `text`
/// ends. The image keeps some texts many times over, so they are matched by what they say.
fn naming(image: &Image, text: &str, start: u64, code: &[u8]) -> Vec<usize> {
    let wanted = format!("{text}\0");
    (0..code.len().saturating_sub(LEA_LEN))
        .filter(|&at| {
            code[at] & 0xF8 == 0x48 && code[at + 1] == LEA && code[at + 2] & 0xC7 == RIP_RELATIVE
        })
        .filter_map(|at| {
            let end = at + LEA_LEN;
            let rel = i64::from(read_u32_le(code, at + 3)?.cast_signed());
            let target = start.checked_add_signed(i64::try_from(end).ok()? + rel)?;
            (image.code(target, wanted.len())? == wanted.as_bytes()).then_some(end)
        })
        .collect()
}

/// The 32-bit `mov reg, [base+disp32]` loads in `code`, in order.
fn colour_loads(code: &[u8]) -> Vec<Access> {
    let mut loads = Vec::new();
    let mut at = 0;
    while at < code.len() {
        match access(code, at) {
            Some(load) if load.op == MOV_LOAD && load.rex & REX_W == 0 => {
                at += load.len;
                loads.push(load);
            }
            _ => at += 1,
        }
    }
    loads
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

/// The maxima a function stores in a run beside the numbers, health and the `others`: health,
/// shields, how far over that shields can go, and overguard.
fn maxima(code: &[u8], health: u32, others: &[u32]) -> Option<[u32; MAXIMA_RUN]> {
    (0..code.len()).find_map(|at| {
        let mut stores = Vec::with_capacity(MAXIMA_RUN);
        let mut next = at;
        while stores.len() < MAXIMA_RUN {
            let store = access(code, next)?;
            let near = store.disp > health && store.disp - health <= MAXIMA_NEAR;
            if store.op != MOV_STORE
                || store.rex & REX_W != 0
                || !near
                || others.contains(&store.disp)
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
        let others = [0x2dc, 0x2ec];
        assert_eq!(
            maxima(&stores, 0x2d8, &others),
            Some([0x2e0, 0x2e4, 0x2e8, 0x2f0])
        );
        assert_eq!(maxima(&stores[7..], 0x2d8, &others), None);
        assert_eq!(maxima(&stores, 0x2d8, &[0x2e4]), None);
    }

    #[test]
    fn finds_the_panels_colours() {
        const METHOD: u64 = 0x1_4000_0000;
        // lea rdx, [rip+mask] at 0; mov edx, [r14+0x1ec]; mov ecx, [rbx+0x10];
        // mov eax, [r14+0x1f4]; mov r8d, [r14+0x1f0]
        let mut code = vec![0x48, 0x8D, 0x15, 0, 0, 0, 0];
        code.extend([0x41, 0x8B, 0x96, 0xEC, 0x01, 0, 0]);
        code.extend([0x8B, 0x8B, 0x10, 0, 0, 0]);
        code.extend([0x41, 0x8B, 0x86, 0xF4, 0x01, 0, 0]);
        code.extend([0x45, 0x8B, 0x86, 0xF0, 0x01, 0, 0]);
        // mov r8d, [r14+0x208]; lea rdx, [rip+hex]
        code.extend([0x45, 0x8B, 0x86, 0x08, 0x02, 0, 0]);
        let hex = code.len();
        code.extend([0x48, 0x8D, 0x15, 0, 0, 0, 0]);
        // The texts follow the code; each `lea` points at its own.
        let point = |code: &mut Vec<u8>, lea: usize, text: &[u8]| {
            let rel = u32::try_from(code.len() - (lea + LEA_LEN)).expect("near");
            code[lea + 3..lea + LEA_LEN].copy_from_slice(&rel.to_le_bytes());
            code.extend(text);
        };
        point(&mut code, 0, b"DotPreviewFillMask\0");
        let without_hex = Image::from_bytes(METHOD, code.clone());
        point(&mut code, hex, b"%02X%02X%02X\0");
        let image = Image::from_bytes(METHOD, code);
        assert_eq!(
            panel_colours(&image, METHOD),
            Some(PanelColours {
                health: 0x1ec,
                shield: 0x1f0,
                overshield: 0x1f4,
                overguard: 0x208,
            })
        );
        assert_eq!(panel_colours(&without_hex, METHOD), None);
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
            (0x1ec, 0xcc_2929),
            (0x1f0, 0x00_d5ff),
            (0x1f4, 0xb3_00ff),
            (0x208, 0x1_000_000),
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
                health_colour: Some(0xcc_2929),
                shield_colour: Some(0x00_d5ff),
                overshield_colour: Some(0xb3_00ff),
                overguard_colour: None,
            })
        );
        let unset = HudVitals::at(0x500, 0x6b0, 0x2e8);
        assert_eq!(unset.read(&reader, hud), None);
    }
}
