/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Load-time fixes for guest code that depends on behaviour the emulated
//! environment cannot reproduce. Each fix is located by an exact byte
//! signature (never a hard-coded address) and applied only when that
//! signature appears exactly once in the executable's `__text` section.

use crate::mach_o::MachO;
use crate::mem::{Mem, Ptr};

/// CRI Mana 1.35 (Miku Flick): `CriMvEasyPlayer::executeWaitStatusServer`
/// keeps a movie in PLAYING after the inner player reached PLAYEND until the
/// playback clock passes `(frames + 1) / fps`. The clock follows the movie's
/// audio, and in Miku Flick's PVs the audio ends several seconds before that
/// time ("VOC@LOID in Love": 3677 frames @ 20 fps = 183.9 s, music 2:56). The
/// clock stops with the audio, the check never passes, cleanup never runs and
/// the game waits on a black frame forever after the song.
///
/// The inner player only reports PLAYEND once every video frame was decoded
/// and every audio track ended, so the time check is redundant there. The fix
/// turns its `ble` into an unconditional `b` to the cleanup path.
///
/// Signature: `vmul.f32 d0,d0,d2` ×2, `vcmpe.f32 s0,s2`, `vmrs APSR_nzcv,fpscr`,
/// then `ble` (condition 0xD, the byte patched to 0xE = always).
const CRI_MANA_END_TIME_SIGNATURE: [u8; 16] = [
    0x12, 0x0d, 0x00, 0xf3, 0x12, 0x0d, 0x00, 0xf3, 0xc1, 0x0a, 0xb4, 0xee, 0x10, 0xfa, 0xf1, 0xee,
];

pub fn apply(executable: &MachO, mem: &mut Mem) {
    let Some(text) = executable.sections.iter().find(|s| s.name == "__text") else {
        return;
    };
    let bytes = mem.bytes_at(Ptr::<u8, false>::from_bits(text.addr), text.size);
    let sig = &CRI_MANA_END_TIME_SIGNATURE;
    let mut matches = bytes
        .windows(sig.len() + 4)
        .enumerate()
        .filter(|(_, w)| &w[..sig.len()] == sig && w[sig.len() + 3] == 0xda)
        .map(|(offset, _)| offset);
    let (Some(offset), None) = (matches.next(), matches.next()) else {
        return;
    };
    let condition_byte = text.addr + (offset + sig.len() + 3) as u32;
    mem.bytes_at_mut(Ptr::from_bits(condition_byte), 1)[0] = 0xea;
    log!(
        "Applying game fix: CRI Mana movie end no longer waits for the audio-driven clock (ble -> b at {:#x}).",
        condition_byte - 3
    );
}
