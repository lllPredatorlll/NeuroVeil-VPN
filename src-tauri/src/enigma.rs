use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use ring::digest::{digest, SHA256};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use std::collections::VecDeque;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;

pub const ENIGMA_KEYBOOK_MAGIC: &[u8; 16] = b"NVENIGMA_KEY_V1\0";
pub const ENIGMA_KEYBOOK_LEN: usize = 16384;
pub const ENIGMA_MASTER_KEY_OFFSET: usize = 16;
pub const ENIGMA_HINT_KEY_OFFSET: usize = 48;
pub const ENIGMA_ROTORS_OFFSET: usize = 80;
pub const ENIGMA_ROTOR_COUNT: usize = 256;
pub const ENIGMA_ROTOR_SIZE: usize = 32;
pub const ENIGMA_CORPUS_OFFSET: usize = 8272; // 80 + 256 * 32
pub const ENIGMA_CORPUS_LEN: usize = 8112; // 16384 - 8272
pub const ENIGMA_TIME_SLOT_SECS: u64 = 30;
pub const ENIGMA_WIRE_HEADER_LEN: usize = 24; // 12B mimic + 12B masked salt
pub const ENIGMA_TAG_LEN: usize = 16;

#[derive(Clone, Debug)]
pub struct EnigmaKeybook {
    pub raw: Vec<u8>,
}

impl EnigmaKeybook {
    pub fn generate() -> Result<Self, String> {
        let rng = SystemRandom::new();
        let mut raw = vec![0u8; ENIGMA_KEYBOOK_LEN];
        raw[0..16].copy_from_slice(ENIGMA_KEYBOOK_MAGIC);

        // Master key + Hint key + 256 rotors = 8256 bytes pure CSPRNG
        rng.fill(&mut raw[16..ENIGMA_CORPUS_OFFSET])
            .map_err(|_| "CSPRNG failure generating Enigma keybook".to_string())?;

        // Build low-entropy realistic corpus for padding (reduces high-entropy DPI triggers)
        let mut rand_pool = vec![0u8; 2048];
        rng.fill(&mut rand_pool)
            .map_err(|_| "CSPRNG failure generating Enigma corpus".to_string())?;

        const TOKENS: &[&[u8]] = &[
            b"GET /cdn-cgi/trace HTTP/1.1\r\nHost: ",
            b"application/octet-stream",
            b"session_ticket=",
            b"quic-v1;h3=\"h3-29\"",
            b"TLS_AES_128_GCM_SHA256",
            b"stun:global.stun.twilio.com",
            b"Cache-Control: no-cache\r\n",
            b"\x00\x00\x00\x01\x00\x01\x00\x00",
        ];

        let mut pos = ENIGMA_CORPUS_OFFSET;
        let mut r_idx = 0usize;
        let mut t_idx = 0usize;
        while pos < ENIGMA_KEYBOOK_LEN {
            let tok = TOKENS[(t_idx + (rand_pool[r_idx % rand_pool.len()] as usize)) % TOKENS.len()];
            let take_tok = tok.len().min(ENIGMA_KEYBOOK_LEN - pos);
            raw[pos..pos + take_tok].copy_from_slice(&tok[..take_tok]);
            pos += take_tok;
            t_idx += 1;

            let zeros = (rand_pool[(r_idx + 1) % rand_pool.len()] as usize % 6) + 2;
            let take_z = zeros.min(ENIGMA_KEYBOOK_LEN - pos);
            for b in &mut raw[pos..pos + take_z] {
                *b = rand_pool[r_idx % rand_pool.len()] & 0x3F;
            }
            pos += take_z;

            let noise = (rand_pool[(r_idx + 2) % rand_pool.len()] as usize % 8) + 2;
            let take_n = noise.min(ENIGMA_KEYBOOK_LEN - pos);
            for i in 0..take_n {
                raw[pos + i] = rand_pool[(r_idx + 3 + i) % rand_pool.len()];
            }
            pos += take_n;
            r_idx += 5;
        }

        Ok(Self { raw })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != ENIGMA_KEYBOOK_LEN {
            return Err(format!(
                "Некорректный размер файла ключа .nvkey: {} байт (ожидается {})",
                bytes.len(),
                ENIGMA_KEYBOOK_LEN
            ));
        }
        if &bytes[0..16] != ENIGMA_KEYBOOK_MAGIC {
            return Err("Неверный заголовок файла ключа .nvkey (ожидается NVENIGMA_KEY_V1)".to_string());
        }
        Ok(Self {
            raw: bytes.to_vec(),
        })
    }

    pub fn from_base64(b64: &str) -> Result<Self, String> {
        let cleaned: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = BASE64
            .decode(cleaned.as_bytes())
            .map_err(|e| format!("Ошибка декодирования Base64 ключа Enigma: {}", e))?;
        Self::from_bytes(&bytes)
    }

    pub fn to_base64(&self) -> String {
        BASE64.encode(&self.raw)
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path)
            .map_err(|e| format!("Не удалось прочитать файл ключа {}: {}", path.display(), e))?;
        Self::from_bytes(&bytes)
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Ошибка создания папки {}: {}", parent.display(), e))?;
        }
        fs::write(path, &self.raw)
            .map_err(|e| format!("Ошибка записи файла ключа {}: {}", path.display(), e))
    }

    #[inline]
    pub fn master_key(&self) -> &[u8] {
        &self.raw[ENIGMA_MASTER_KEY_OFFSET..ENIGMA_MASTER_KEY_OFFSET + 32]
    }

    #[inline]
    pub fn hint_key(&self) -> &[u8] {
        &self.raw[ENIGMA_HINT_KEY_OFFSET..ENIGMA_HINT_KEY_OFFSET + 32]
    }

    #[inline]
    pub fn rotor(&self, idx: u8) -> &[u8] {
        let start = ENIGMA_ROTORS_OFFSET + (idx as usize) * ENIGMA_ROTOR_SIZE;
        &self.raw[start..start + ENIGMA_ROTOR_SIZE]
    }

    #[inline]
    pub fn copy_corpus(&self, start_offset: usize, dst: &mut [u8]) {
        if dst.is_empty() {
            return;
        }
        let corpus = &self.raw[ENIGMA_CORPUS_OFFSET..ENIGMA_KEYBOOK_LEN];
        let mut idx = start_offset % ENIGMA_CORPUS_LEN;
        for b in dst.iter_mut() {
            *b = corpus[idx];
            idx += 1;
            if idx >= ENIGMA_CORPUS_LEN {
                idx = 0;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MimicMode {
    Auto = 255,
    Quic = 0,
    Stun = 1,
    Dtls = 2,
    Dns = 3,
}

impl MimicMode {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
            "quic" => MimicMode::Quic,
            "stun" => MimicMode::Stun,
            "dtls" => MimicMode::Dtls,
            "dns" => MimicMode::Dns,
            _ => MimicMode::Auto,
        }
    }

    pub fn label(mode_byte: u8) -> &'static str {
        match mode_byte & 3 {
            0 => "QUIC",
            1 => "STUN",
            2 => "DTLS",
            _ => "DNS",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HandshakeMeta {
    pub r1: u8,
    pub r2: u8,
    pub r3: u8,
    pub mimic_mode: u8,
    pub wire_len: usize,
    pub swapped: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EnigmaTelemetry {
    pub handshakes_sent: u64,
    pub decoys_sent: u64,
    pub last_rotor_summary: Option<String>,
    pub packets_tx: u64,
    pub packets_rx: u64,
}

#[derive(Clone)]
pub struct EnigmaTunnelHandle {
    pub local_wg_port: u16,
    stop_flag: Arc<AtomicBool>,
    pub telemetry: Arc<Mutex<EnigmaTelemetry>>,
}

impl EnigmaTunnelHandle {
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }

    pub fn summary(&self) -> Option<String> {
        self.telemetry
            .lock()
            .ok()
            .and_then(|t| t.last_rotor_summary.clone())
    }

    pub fn snapshot(&self) -> Option<EnigmaTelemetry> {
        self.telemetry.lock().ok().map(|t| t.clone())
    }
}

pub struct EnigmaReplayCache {
    seen: VecDeque<([u8; 12], u64)>,
    capacity: usize,
}

impl EnigmaReplayCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            seen: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn check_and_insert(&mut self, salt: &[u8; 12], now_secs: u64) -> bool {
        while let Some((_, ts)) = self.seen.front() {
            if now_secs.saturating_sub(*ts) > 120 {
                self.seen.pop_front();
            } else {
                break;
            }
        }
        if self.seen.iter().any(|(s, _)| s == salt) {
            return false;
        }
        if self.seen.len() >= self.capacity {
            self.seen.pop_front();
        }
        self.seen.push_back((*salt, now_secs));
        true
    }
}

fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let k = hmac::Key::new(hmac::HMAC_SHA256, key);
    let mut ctx = hmac::Context::with_key(&k);
    for p in parts {
        ctx.update(p);
    }
    let tag = ctx.sign();
    let mut out = [0u8; 32];
    out.copy_from_slice(tag.as_ref());
    out
}

fn build_mimic_header(mode: u8, r: &[u8; 8], approx_body_len: usize) -> [u8; 12] {
    let len16 = approx_body_len.min(65535) as u16;
    let hi = (len16 >> 8) as u8;
    let lo = (len16 & 0xFF) as u8;
    match mode & 3 {
        0 => [
            0xC0 | (r[0] & 0x3F),
            0x00,
            0x00,
            0x00,
            0x01,
            0x08,
            r[1],
            r[2],
            r[3],
            r[4],
            r[5],
            r[6],
        ],
        1 => [
            0x00, 0x01, hi, lo, 0x21, 0x12, 0xA4, 0x42, r[0], r[1], r[2], r[3],
        ],
        2 => [
            0x17, 0xFE, 0xFD, 0x00, 0x01, r[0], r[1], r[2], r[3], r[4], hi, lo,
        ],
        _ => [
            r[0], r[1], 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
    }
}

fn compute_salt_mask(hint_key: &[u8], mimic: &[u8; 12]) -> [u8; 12] {
    let mut buf = [0u8; 44];
    buf[0..32].copy_from_slice(hint_key);
    buf[32..44].copy_from_slice(mimic);
    let d = digest(&SHA256, &buf);
    let mut out = [0u8; 12];
    out.copy_from_slice(&d.as_ref()[0..12]);
    out
}

fn aead_seal_in_place(key_bytes: &[u8; 32], nonce_bytes: &[u8; 12], aad: &[u8], buf: &mut Vec<u8>) -> Result<(), String> {
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, key_bytes)
        .map_err(|_| "Invalid AEAD key".to_string())?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce_bytes);
    key.seal_in_place_append_tag(nonce, Aad::from(aad), buf)
        .map_err(|_| "AEAD seal failed".to_string())
}

fn aead_open_in_place<'a>(
    key_bytes: &[u8; 32],
    nonce_bytes: &[u8; 12],
    aad: &[u8],
    buf: &'a mut [u8],
) -> Result<&'a [u8], String> {
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, key_bytes)
        .map_err(|_| "Invalid AEAD key".to_string())?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce_bytes);
    let plain = key
        .open_in_place(nonce, Aad::from(aad), buf)
        .map_err(|_| "AEAD auth failed".to_string())?;
    Ok(plain)
}

pub fn derive_data_key(
    keybook: &EnigmaKeybook,
    r1: u8,
    r2: u8,
    r3: u8,
    session_id: u32,
) -> [u8; 32] {
    let sid_be = session_id.to_be_bytes();
    hmac_sha256(
        keybook.master_key(),
        &[
            b"ENIGMA_V1_DATA",
            keybook.rotor(r1),
            keybook.rotor(r2),
            keybook.rotor(r3),
            &sid_be,
        ],
    )
}

fn derive_rotor_and_hs_key(
    keybook: &EnigmaKeybook,
    slot: u64,
    salt: &[u8; 12],
) -> ([u8; 32], u8, u8, u8, [u8; 32]) {
    let slot_be = slot.to_be_bytes();
    let rot_hash = hmac_sha256(
        keybook.master_key(),
        &[b"ENIGMA_V1_ROT", &slot_be, salt],
    );
    let r1 = rot_hash[0];
    let r2 = rot_hash[1];
    let r3 = rot_hash[2];
    let hs_key = hmac_sha256(
        keybook.master_key(),
        &[
            keybook.rotor(r1),
            keybook.rotor(r2),
            keybook.rotor(r3),
            &slot_be,
            salt,
        ],
    );
    (rot_hash, r1, r2, r3, hs_key)
}

pub fn seal_handshake_with_salt(
    keybook: &EnigmaKeybook,
    wg_pkt: &[u8],
    session_id: u32,
    rand8: &[u8; 8],
    now_secs: u64,
    mimic_pref: MimicMode,
) -> Result<(Vec<u8>, [u8; 32], HandshakeMeta), String> {
    if wg_pkt.len() < 4 || wg_pkt.len() > 2048 {
        return Err("Invalid inner handshake packet length".to_string());
    }
    let mut salt = [0u8; 12];
    salt[0..4].copy_from_slice(&session_id.to_be_bytes());
    salt[4..12].copy_from_slice(rand8);

    let slot = now_secs / ENIGMA_TIME_SLOT_SECS;
    let (rot_hash, r1, r2, r3, hs_key) = derive_rotor_and_hs_key(keybook, slot, &salt);
    let data_key = derive_data_key(keybook, r1, r2, r3, session_id);

    let swap_flag = (rot_hash[4] & 1) != 0;
    let pre_pad_len = (rot_hash[5] as usize) % 96;
    let mid_pad_len = (rot_hash[6] as usize) % 96;
    let post_pad_len = 16 + ((rot_hash[7] as usize) % 140);
    let split_pos = if wg_pkt.len() >= 32 {
        16 + ((rot_hash[3] as usize) % (wg_pkt.len() - 31))
    } else {
        wg_pkt.len() / 2
    };
    let part_a = &wg_pkt[..split_pos];
    let part_b = &wg_pkt[split_pos..];

    let inner_len = 7 + pre_pad_len + wg_pkt.len() + mid_pad_len + post_pad_len;
    let total_approx = 12 + inner_len + ENIGMA_TAG_LEN;

    let mimic_mode = match mimic_pref {
        MimicMode::Auto => rot_hash[11] & 3,
        other => (other as u8) & 3,
    };
    let mimic = build_mimic_header(mimic_mode, rand8, total_approx);
    let mask = compute_salt_mask(keybook.hint_key(), &mimic);

    let mut hdr = [0u8; ENIGMA_WIRE_HEADER_LEN];
    hdr[0..12].copy_from_slice(&mimic);
    for i in 0..12 {
        hdr[12 + i] = salt[i] ^ mask[i];
    }

    let mut body = Vec::with_capacity(inner_len + ENIGMA_TAG_LEN);
    let flags = (if swap_flag { 1u8 } else { 0u8 }) | (rot_hash[8] & 0xFE);
    body.push(flags);
    body.push(pre_pad_len as u8);
    body.push(mid_pad_len as u8);
    body.extend_from_slice(&(part_a.len() as u16).to_be_bytes());
    body.extend_from_slice(&(part_b.len() as u16).to_be_bytes());

    let c0 = u16::from_be_bytes([rot_hash[9], rot_hash[10]]) as usize;
    let mut pad_tmp = vec![0u8; pre_pad_len.max(mid_pad_len).max(post_pad_len)];

    if pre_pad_len > 0 {
        keybook.copy_corpus(c0, &mut pad_tmp[..pre_pad_len]);
        body.extend_from_slice(&pad_tmp[..pre_pad_len]);
    }
    if swap_flag {
        body.extend_from_slice(part_b);
    } else {
        body.extend_from_slice(part_a);
    }
    if mid_pad_len > 0 {
        keybook.copy_corpus(c0.wrapping_add(137), &mut pad_tmp[..mid_pad_len]);
        body.extend_from_slice(&pad_tmp[..mid_pad_len]);
    }
    if swap_flag {
        body.extend_from_slice(part_a);
    } else {
        body.extend_from_slice(part_b);
    }
    if post_pad_len > 0 {
        keybook.copy_corpus(c0.wrapping_add(509), &mut pad_tmp[..post_pad_len]);
        body.extend_from_slice(&pad_tmp[..post_pad_len]);
    }

    aead_seal_in_place(&hs_key, &salt, &hdr, &mut body)?;

    let mut out = Vec::with_capacity(ENIGMA_WIRE_HEADER_LEN + body.len());
    out.extend_from_slice(&hdr);
    out.extend_from_slice(&body);
    let wire_len = out.len();

    Ok((
        out,
        data_key,
        HandshakeMeta {
            r1,
            r2,
            r3,
            mimic_mode,
            wire_len,
            swapped: swap_flag,
        },
    ))
}

pub fn open_handshake(
    keybook: &EnigmaKeybook,
    wire_pkt: &[u8],
    now_secs: u64,
) -> Result<(Vec<u8>, u32, [u8; 12], [u8; 32]), String> {
    if wire_pkt.len() < ENIGMA_WIRE_HEADER_LEN + 7 + ENIGMA_TAG_LEN {
        return Err("Enigma handshake packet too short".to_string());
    }
    let mut mimic = [0u8; 12];
    mimic.copy_from_slice(&wire_pkt[0..12]);
    let mask = compute_salt_mask(keybook.hint_key(), &mimic);
    let mut salt = [0u8; 12];
    for i in 0..12 {
        salt[i] = wire_pkt[12 + i] ^ mask[i];
    }
    let session_id = u32::from_be_bytes([salt[0], salt[1], salt[2], salt[3]]);
    let cur_slot = now_secs / ENIGMA_TIME_SLOT_SECS;
    let candidate_slots = [cur_slot, cur_slot.saturating_sub(1), cur_slot.saturating_add(1)];

    for slot in candidate_slots {
        let (_, r1, r2, r3, hs_key) = derive_rotor_and_hs_key(keybook, slot, &salt);
        let mut cipher_buf = wire_pkt[ENIGMA_WIRE_HEADER_LEN..].to_vec();
        if let Ok(plain) = aead_open_in_place(&hs_key, &salt, &wire_pkt[0..ENIGMA_WIRE_HEADER_LEN], &mut cipher_buf) {
            if plain.len() < 7 {
                return Err("Malformed Enigma handshake inner".to_string());
            }
            let swapped = (plain[0] & 1) != 0;
            let pre_len = plain[1] as usize;
            let mid_len = plain[2] as usize;
            let a_len = u16::from_be_bytes([plain[3], plain[4]]) as usize;
            let b_len = u16::from_be_bytes([plain[5], plain[6]]) as usize;
            let needed = 7 + pre_len + a_len + mid_len + b_len;
            if plain.len() < needed || a_len + b_len == 0 || a_len + b_len > 2048 {
                return Err("Corrupted Enigma handshake framing".to_string());
            }
            let p1_start = 7 + pre_len;
            let (part_a, part_b) = if swapped {
                let b_slice = &plain[p1_start..p1_start + b_len];
                let a_start = p1_start + b_len + mid_len;
                let a_slice = &plain[a_start..a_start + a_len];
                (a_slice, b_slice)
            } else {
                let a_slice = &plain[p1_start..p1_start + a_len];
                let b_start = p1_start + a_len + mid_len;
                let b_slice = &plain[b_start..b_start + b_len];
                (a_slice, b_slice)
            };
            let mut wg = Vec::with_capacity(a_len + b_len);
            wg.extend_from_slice(part_a);
            wg.extend_from_slice(part_b);
            let data_key = derive_data_key(keybook, r1, r2, r3, session_id);
            return Ok((wg, session_id, salt, data_key));
        }
    }

    Err("Enigma handshake authentication failed".to_string())
}

pub fn seal_data_packet(
    keybook: &EnigmaKeybook,
    data_key: &[u8; 32],
    session_id: u32,
    seq: u64,
    wg_pkt: &[u8],
    mimic_pref: MimicMode,
) -> Result<Vec<u8>, String> {
    if wg_pkt.len() > 2048 {
        return Err("Inner WireGuard packet too large".to_string());
    }
    let mut salt = [0u8; 12];
    salt[0..4].copy_from_slice(&session_id.to_be_bytes());
    salt[4..12].copy_from_slice(&seq.to_be_bytes());

    let rot_byte = keybook.rotor((seq as u8) ^ ((session_id & 0xFF) as u8))[((seq >> 8) as usize) & 31];
    let pad_len = if wg_pkt.len() <= 128 {
        24 + ((rot_byte as usize) % 120)
    } else if wg_pkt.len() <= 1100 {
        8 + ((rot_byte as usize) % 32)
    } else {
        (rot_byte as usize) % 12
    };

    let mimic_mode = match mimic_pref {
        MimicMode::Auto => (rot_byte >> 2) & 3,
        other => (other as u8) & 3,
    };
    let approx_len = 12 + 2 + wg_pkt.len() + pad_len + ENIGMA_TAG_LEN;
    let mimic_rand: [u8; 8] = salt[4..12].try_into().unwrap_or([0u8; 8]);
    let mimic = build_mimic_header(mimic_mode, &mimic_rand, approx_len);
    let mask = compute_salt_mask(keybook.hint_key(), &mimic);

    let mut hdr = [0u8; ENIGMA_WIRE_HEADER_LEN];
    hdr[0..12].copy_from_slice(&mimic);
    for i in 0..12 {
        hdr[12 + i] = salt[i] ^ mask[i];
    }

    let mut body = Vec::with_capacity(2 + wg_pkt.len() + pad_len + ENIGMA_TAG_LEN);
    body.extend_from_slice(&(wg_pkt.len() as u16).to_be_bytes());
    body.extend_from_slice(wg_pkt);
    if pad_len > 0 {
        let old_len = body.len();
        body.resize(old_len + pad_len, 0);
        let c_off = ((seq as usize).wrapping_mul(37)).wrapping_add(rot_byte as usize);
        keybook.copy_corpus(c_off, &mut body[old_len..]);
    }

    aead_seal_in_place(data_key, &salt, &hdr, &mut body)?;
    let mut out = Vec::with_capacity(ENIGMA_WIRE_HEADER_LEN + body.len());
    out.extend_from_slice(&hdr);
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn open_data_packet(
    keybook: &EnigmaKeybook,
    data_key: &[u8; 32],
    wire_pkt: &[u8],
) -> Result<(Vec<u8>, u32, u64), String> {
    if wire_pkt.len() < ENIGMA_WIRE_HEADER_LEN + 2 + ENIGMA_TAG_LEN {
        return Err("Enigma data packet too short".to_string());
    }
    let mut mimic = [0u8; 12];
    mimic.copy_from_slice(&wire_pkt[0..12]);
    let mask = compute_salt_mask(keybook.hint_key(), &mimic);
    let mut salt = [0u8; 12];
    for i in 0..12 {
        salt[i] = wire_pkt[12 + i] ^ mask[i];
    }
    let session_id = u32::from_be_bytes([salt[0], salt[1], salt[2], salt[3]]);
    let seq = u64::from_be_bytes([
        salt[4], salt[5], salt[6], salt[7], salt[8], salt[9], salt[10], salt[11],
    ]);

    let mut cipher_buf = wire_pkt[ENIGMA_WIRE_HEADER_LEN..].to_vec();
    let plain = aead_open_in_place(data_key, &salt, &wire_pkt[0..ENIGMA_WIRE_HEADER_LEN], &mut cipher_buf)?;
    if plain.len() < 2 {
        return Err("Enigma data packet missing length".to_string());
    }
    let orig_len = u16::from_be_bytes([plain[0], plain[1]]) as usize;
    if orig_len == 0 || plain.len() < 2 + orig_len {
        return Err("Invalid inner WireGuard length".to_string());
    }
    Ok((plain[2..2 + orig_len].to_vec(), session_id, seq))
}

pub fn build_decoy_packet(
    keybook: &EnigmaKeybook,
    rand_bytes: &[u8; 16],
    mimic_pref: MimicMode,
) -> Vec<u8> {
    let body_len = 140 + (((rand_bytes[0] as usize) * 3) % 420);
    let mode = match mimic_pref {
        MimicMode::Auto => rand_bytes[1] & 3,
        other => (other as u8) & 3,
    };
    let mut r8 = [0u8; 8];
    r8.copy_from_slice(&rand_bytes[2..10]);
    let mimic = build_mimic_header(mode, &r8, body_len);

    let mut out = vec![0u8; body_len];
    out[0..12].copy_from_slice(&mimic);
    let mask = compute_salt_mask(keybook.hint_key(), &mimic);
    for i in 0..12 {
        out[12 + i] = rand_bytes[(4 + i) % 16] ^ mask[i];
    }
    let corpus_off = u16::from_be_bytes([rand_bytes[10], rand_bytes[11]]) as usize;
    keybook.copy_corpus(corpus_off, &mut out[24..]);
    // Light stream mask over first 32 bytes after header so decoys look indistinguishable from AEAD frames
    for i in 24..56.min(body_len) {
        out[i] ^= rand_bytes[i % 16].wrapping_add(i as u8);
    }
    out
}

#[inline]
fn is_wg_handshake_packet(pkt: &[u8]) -> bool {
    if pkt.len() < 4 {
        return false;
    }
    let t = pkt[0];
    pkt[1] == 0 && pkt[2] == 0 && pkt[3] == 0 && (t == 1 || t == 2 || t == 3)
}

#[inline]
fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn ensure_profile_keybook(
    app_data_dir: &Path,
    profile_id: &str,
    b64_opt: Option<&str>,
    path_opt: Option<&str>,
) -> Result<(EnigmaKeybook, String), String> {
    if let Some(b64) = b64_opt {
        if !b64.trim().is_empty() {
            let kb = EnigmaKeybook::from_base64(b64)?;
            let key_dir = app_data_dir.join("enigma_keys");
            let file_path = key_dir.join(format!("{}.nvkey", profile_id));
            let _ = kb.save_to_file(&file_path);
            return Ok((kb, file_path.to_string_lossy().to_string()));
        }
    }
    if let Some(p_str) = path_opt {
        let trimmed = p_str.trim();
        if !trimmed.is_empty() {
            let p = Path::new(trimmed);
            if p.exists() {
                let kb = EnigmaKeybook::from_file(p)?;
                return Ok((kb, trimmed.to_string()));
            }
        }
    }
    let fallback_path = app_data_dir
        .join("enigma_keys")
        .join(format!("{}.nvkey", profile_id));
    if fallback_path.exists() {
        let kb = EnigmaKeybook::from_file(&fallback_path)?;
        return Ok((kb, fallback_path.to_string_lossy().to_string()));
    }
    Err("Для профиля Enigma не найден файл ключа (.nvkey). Импортируйте .nvkey или перевыпустите профиль с VPS.".to_string())
}

pub fn start_enigma_client_tunnel(
    server_host: &str,
    server_port: u16,
    keybook: EnigmaKeybook,
    mimic_mode: MimicMode,
    runtime_log: Arc<Mutex<Vec<String>>>,
) -> Result<EnigmaTunnelHandle, String> {
    let target_addr: SocketAddr = format!("{}:{}", server_host.trim(), server_port)
        .parse()
        .or_else(|_| {
            use std::net::ToSocketAddrs;
            format!("{}:{}", server_host.trim(), server_port)
                .to_socket_addrs()
                .map_err(|e| format!("DNS error for Enigma server: {}", e))?
                .find(|a| a.is_ipv4())
                .ok_or_else(|| "Не найден IPv4-адрес сервера Enigma".to_string())
        })?;

    let std_local = std::net::UdpSocket::bind("127.0.0.1:0")
        .map_err(|e| format!("Не удалось открыть локальный UDP-порт Enigma: {}", e))?;
    std_local
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking local error: {}", e))?;
    let local_wg_port = std_local
        .local_addr()
        .map_err(|e| format!("local_addr error: {}", e))?
        .port();

    let std_remote = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Не удалось открыть внешний UDP-сокет Enigma: {}", e))?;
    std_remote
        .connect(target_addr)
        .map_err(|e| format!("Ошибка connect() внешнего UDP-сокета Enigma: {}", e))?;
    std_remote
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking remote error: {}", e))?;

    let rng = SystemRandom::new();
    let mut sid_bytes = [0u8; 4];
    rng.fill(&mut sid_bytes)
        .map_err(|_| "CSPRNG failure generating Enigma session_id".to_string())?;
    let session_id = u32::from_be_bytes(sid_bytes);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let telemetry = Arc::new(Mutex::new(EnigmaTelemetry::default()));
    let telemetry_clone = telemetry.clone();

    let push_log = move |msg: String| {
        if let Ok(mut logs) = runtime_log.lock() {
            logs.push(msg);
            if logs.len() > 400 {
                let excess = logs.len() - 400;
                logs.drain(0..excess);
            }
        }
    };

    push_log(format!(
        "[Enigma] Локальный роторный шлюз 127.0.0.1:{} -> {} (session {:08X}, mimic={:?})",
        local_wg_port, target_addr, session_id, mimic_mode
    ));

    std::thread::Builder::new()
        .name(format!("enigma-tunnel-{}", local_wg_port))
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };

            rt.block_on(async move {
                let Ok(local_sock) = UdpSocket::from_std(std_local) else { return; };
                let Ok(remote_sock) = UdpSocket::from_std(std_remote) else { return; };

                let seq_counter = AtomicU64::new(1);
                let mut cur_data_key: Option<[u8; 32]> = None;
                let mut prev_data_key: Option<[u8; 32]> = None;
                let mut last_local_peer: Option<SocketAddr> = None;
                let mut replay_cache = EnigmaReplayCache::new(512);

                let mut local_buf = vec![0u8; 4096];
                let mut remote_buf = vec![0u8; 4096];
                let mut tick = tokio::time::interval(Duration::from_millis(200));

                loop {
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }

                    tokio::select! {
                        _ = tick.tick() => {
                            if stop_clone.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                        res = local_sock.recv_from(&mut local_buf) => {
                            let Ok((n, peer)) = res else { continue; };
                            if n == 0 { continue; }
                            last_local_peer = Some(peer);
                            let wg_pkt = &local_buf[..n];

                            if is_wg_handshake_packet(wg_pkt) || cur_data_key.is_none() {
                                let mut rand_decoy = [0u8; 16];
                                let _ = rng.fill(&mut rand_decoy);
                                let decoy_count = 1 + (rand_decoy[0] % 3); // 1..3 decoy packets per handshake
                                for d_idx in 0..decoy_count {
                                    let mut d_rand = [0u8; 16];
                                    let _ = rng.fill(&mut d_rand);
                                    let decoy = build_decoy_packet(&keybook, &d_rand, mimic_mode);
                                    let _ = remote_sock.send(&decoy).await;
                                    if d_idx + 1 < decoy_count {
                                        let jitter_ms = 2 + (d_rand[15] as u64 % 6);
                                        tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
                                    }
                                }

                                let mut rand8 = [0u8; 8];
                                let _ = rng.fill(&mut rand8);
                                let now = unix_now_secs();
                                if let Ok((wire_pkt, new_dk, meta)) = seal_handshake_with_salt(
                                    &keybook,
                                    wg_pkt,
                                    session_id,
                                    &rand8,
                                    now,
                                    mimic_mode,
                                ) {
                                    if wg_pkt[0] == 1 || cur_data_key.is_none() {
                                        prev_data_key = cur_data_key;
                                        cur_data_key = Some(new_dk);
                                    }
                                    let _ = remote_sock.send(&wire_pkt).await;
                                    if let Ok(mut tel) = telemetry_clone.lock() {
                                        tel.handshakes_sent += 1;
                                        tel.decoys_sent += decoy_count as u64;
                                        tel.packets_tx += 1;
                                        tel.last_rotor_summary = Some(format!(
                                            "Роторы [{:03}·{:03}·{:03}] • {} • {}B{} • +{} decoy",
                                            meta.r1,
                                            meta.r2,
                                            meta.r3,
                                            MimicMode::label(meta.mimic_mode),
                                            meta.wire_len,
                                            if meta.swapped { "⇄" } else { "" },
                                            decoy_count
                                        ));
                                    }
                                    push_log(format!(
                                        "[Enigma] Рукопожатие #{}: роторы [{:03}·{:03}·{:03}], мимикрия={}, {}B, swap={}, decoy={}",
                                        telemetry_clone.lock().map(|t| t.handshakes_sent).unwrap_or(1),
                                        meta.r1, meta.r2, meta.r3,
                                        MimicMode::label(meta.mimic_mode),
                                        meta.wire_len,
                                        meta.swapped,
                                        decoy_count
                                    ));
                                }
                            } else if let Some(dk) = cur_data_key {
                                let seq = seq_counter.fetch_add(1, Ordering::Relaxed);
                                if let Ok(wire_pkt) = seal_data_packet(&keybook, &dk, session_id, seq, wg_pkt, mimic_mode) {
                                    let _ = remote_sock.send(&wire_pkt).await;
                                    if let Ok(mut tel) = telemetry_clone.lock() {
                                        tel.packets_tx += 1;
                                    }
                                }
                            }
                        }
                        res = remote_sock.recv(&mut remote_buf) => {
                            let Ok(n) = res else { continue; };
                            if n < ENIGMA_WIRE_HEADER_LEN + 2 + ENIGMA_TAG_LEN { continue; }
                            let wire_pkt = &remote_buf[..n];

                            let mut decoded: Option<Vec<u8>> = None;
                            if let Some(dk) = cur_data_key {
                                if let Ok((wg, sid, _)) = open_data_packet(&keybook, &dk, wire_pkt) {
                                    if sid == session_id {
                                        decoded = Some(wg);
                                    }
                                }
                            }
                            if decoded.is_none() {
                                if let Some(pdk) = prev_data_key {
                                    if let Ok((wg, sid, _)) = open_data_packet(&keybook, &pdk, wire_pkt) {
                                        if sid == session_id {
                                            decoded = Some(wg);
                                        }
                                    }
                                }
                            }
                            if decoded.is_none() {
                                let now = unix_now_secs();
                                if let Ok((wg, sid, salt, _)) = open_handshake(&keybook, wire_pkt, now) {
                                    if sid == session_id && replay_cache.check_and_insert(&salt, now) {
                                        decoded = Some(wg);
                                    }
                                }
                            }

                            if let Some(wg) = decoded {
                                if let Some(peer) = last_local_peer {
                                    let _ = local_sock.send_to(&wg, peer).await;
                                    if let Ok(mut tel) = telemetry_clone.lock() {
                                        tel.packets_rx += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            });
        })
        .map_err(|e| format!("Failed to spawn Enigma thread: {}", e))?;

    Ok(EnigmaTunnelHandle {
        local_wg_port,
        stop_flag,
        telemetry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybook_and_polymorphic_roundtrip() {
        let kb = EnigmaKeybook::generate().unwrap();
        assert_eq!(kb.raw.len(), ENIGMA_KEYBOOK_LEN);
        let b64 = kb.to_base64();
        let kb2 = EnigmaKeybook::from_base64(&b64).unwrap();
        assert_eq!(kb.raw, kb2.raw);

        let mut wg_init = vec![0u8; 148];
        wg_init[0] = 1;
        for (i, b) in wg_init[4..].iter_mut().enumerate() {
            *b = ((i * 7 + 13) & 0xFF) as u8;
        }

        let now = 1795600000u64;
        let sid = 0xDEADBEEFu32;
        let r1 = [1, 2, 3, 4, 5, 6, 7, 8];
        let r2 = [9, 10, 11, 12, 13, 14, 15, 16];

        let (w1, dk1, m1) =
            seal_handshake_with_salt(&kb2, &wg_init, sid, &r1, now, MimicMode::Auto).unwrap();
        let (w2, dk2, m2) =
            seal_handshake_with_salt(&kb2, &wg_init, sid, &r2, now, MimicMode::Auto).unwrap();

        // Identical input produces completely different wire packets and lengths
        assert_ne!(w1, w2);
        assert_ne!(dk1, dk2);
        assert_ne!(w1[0..24], w2[0..24]);

        let (dec1, sid1, salt1, dec_dk1) = open_handshake(&kb, &w1, now + 5).unwrap();
        let (dec2, sid2, _salt2, dec_dk2) = open_handshake(&kb, &w2, now).unwrap();
        assert_eq!(dec1, wg_init);
        assert_eq!(dec2, wg_init);
        assert_eq!(sid1, sid);
        assert_eq!(sid2, sid);
        assert_eq!(dec_dk1, dk1);
        assert_eq!(dec_dk2, dk2);

        // Replay cache blocks duplicated salt
        let mut cache = EnigmaReplayCache::new(32);
        assert!(cache.check_and_insert(&salt1, now));
        assert!(!cache.check_and_insert(&salt1, now + 2));

        // Data frame roundtrip + keepalive padding expansion
        let keepalive = vec![4u8, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let data_wire = seal_data_packet(&kb, &dk1, sid, 42, &keepalive, MimicMode::Auto).unwrap();
        assert!(data_wire.len() > 80); // Keepalive padded dynamically
        let (dec_ka, dec_sid, dec_seq) = open_data_packet(&kb2, &dec_dk1, &data_wire).unwrap();
        assert_eq!(dec_ka, keepalive);
        assert_eq!(dec_sid, sid);
        assert_eq!(dec_seq, 42);

        // Decoy must NOT open as handshake or data
        let decoy = build_decoy_packet(&kb, &[77u8; 16], MimicMode::Auto);
        assert!(open_handshake(&kb, &decoy, now).is_err());
        assert!(open_data_packet(&kb, &dk1, &decoy).is_err());

        // Silence unused warning on m1/m2
        assert!(m1.wire_len >= 160 && m2.wire_len >= 160);
    }

    #[test]
    fn test_rust_c_cross_verification() {
        use std::process::Command;

        let c_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("enigma_srv.c");
        if !c_src.exists() {
            panic!("Missing enigma_srv.c at {:?}", c_src);
        }
        let tmp_dir = std::env::temp_dir().join("neuroveil_enigma_cross_test");
        let _ = fs::create_dir_all(&tmp_dir);
        let exe_path = tmp_dir.join(if cfg!(windows) {
            "enigma_cross_test.exe"
        } else {
            "enigma_cross_test"
        });

        let compile = Command::new("gcc")
            .args([
                "-O2",
                "-std=c99",
                "-Wall",
                "-Wextra",
                "-DENIGMA_SELF_TEST",
                "-o",
                exe_path.to_str().unwrap(),
                c_src.to_str().unwrap(),
            ])
            .output();

        let Ok(comp_out) = compile else {
            eprintln!("Skipping C cross-test: gcc not available in PATH");
            return;
        };
        assert!(
            comp_out.status.success(),
            "gcc compile failed: {}",
            String::from_utf8_lossy(&comp_out.stderr)
        );

        let kb = EnigmaKeybook::generate().unwrap();
        let kb_path = tmp_dir.join("test.nvkey");
        kb.save_to_file(&kb_path).unwrap();

        let mut wg_init = vec![0u8; 148];
        wg_init[0] = 1;
        for (i, b) in wg_init[4..].iter_mut().enumerate() {
            *b = ((i * 11 + 5) & 0xFF) as u8;
        }
        let wg_data = vec![4u8, 0, 0, 0, 9, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let now = 1795601234u64;
        let sid = 0xCAFEBABEu32;
        let rand8 = [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x18];

        let (r_hs, r_dk, _) =
            seal_handshake_with_salt(&kb, &wg_init, sid, &rand8, now, MimicMode::Auto).unwrap();
        let r_dt = seal_data_packet(&kb, &r_dk, sid, 777, &wg_data, MimicMode::Auto).unwrap();

        let hs_path = tmp_dir.join("r_hs.bin");
        let dt_path = tmp_dir.join("r_dt.bin");
        let c_hs_path = tmp_dir.join("c_hs.bin");
        let c_dt_path = tmp_dir.join("c_dt.bin");
        fs::write(&hs_path, &r_hs).unwrap();
        fs::write(&dt_path, &r_dt).unwrap();

        let run_out = Command::new(&exe_path)
            .args([
                kb_path.to_str().unwrap(),
                hs_path.to_str().unwrap(),
                dt_path.to_str().unwrap(),
                &now.to_string(),
                c_hs_path.to_str().unwrap(),
                c_dt_path.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to execute enigma_cross_test");

        let stdout_s = String::from_utf8_lossy(&run_out.stdout);
        let stderr_s = String::from_utf8_lossy(&run_out.stderr);
        assert!(
            run_out.status.success(),
            "C self-test failed: stdout={}, stderr={}",
            stdout_s,
            stderr_s
        );
        assert!(stdout_s.contains("C_CROSS_OK"), "Unexpected stdout: {}", stdout_s);

        let c_hs_bytes = fs::read(&c_hs_path).unwrap();
        let c_dt_bytes = fs::read(&c_dt_path).unwrap();

        let (dec_wg_init, dec_sid, _, dec_dk) = open_handshake(&kb, &c_hs_bytes, now).unwrap();
        let (_, expected_c_dk, _) = seal_handshake_with_salt(
            &kb,
            &wg_init,
            sid,
            &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
            now,
            MimicMode::Auto,
        )
        .unwrap();
        assert_eq!(dec_wg_init, wg_init);
        assert_eq!(dec_sid, sid);
        assert_eq!(dec_dk, expected_c_dk);
        assert_ne!(dec_dk, r_dk);

        let (dec_wg_data, dec_dsid, dec_dseq) = open_data_packet(&kb, &r_dk, &c_dt_bytes).unwrap();
        assert_eq!(dec_wg_data, wg_data);
        assert_eq!(dec_dsid, sid);
        assert_eq!(dec_dseq, 778);
    }
}
