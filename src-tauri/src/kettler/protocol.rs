//! Protocole binaire Kettler, tel que décrit dans CLAUDE.md (source : crate kdri 0.4.3).
//!
//! CONTRAT FIGÉ : les signatures publiques ci-dessous sont utilisées par le
//! module Bluetooth.
//!
//! Trame : `0x02 | payload échappé | 0x03 | CRC_hi CRC_lo (échappés aussi)`.
//! Payload : identifiant u16 BE, instruction u8, longueur u16 BE, données.
//! Module pur : aucune I/O, entièrement testable sans Bluetooth.

/// Octet de début de trame.
const STX: u8 = 0x02;
/// Octet de fin de payload (suivi des 2 octets de CRC).
const ETX: u8 = 0x03;
/// Octet d'échappement : l'octet suivant vaut `octet_réel ^ 0x20`.
const DLE: u8 = 0x10;
/// Masque appliqué à un octet échappé.
const ESC_XOR: u8 = 0x20;
/// Taille de l'en-tête du payload (id 2 + instruction 1 + longueur 2).
const HEADER_LEN: usize = 5;
/// Taille maximale acceptée pour un payload en cours de réception. Les vraies
/// réponses font quelques dizaines d'octets ; au-delà, on considère le flux
/// désynchronisé (0x03 perdu) plutôt que de laisser le tampon grossir sans fin.
const MAX_PAYLOAD: usize = 1024;

/// Payload du message d'identification (valeur n° 1, écriture, 18 octets), tiré de kdri.
const HANDSHAKE_PAYLOAD: [u8; 23] = [
    0x00, 0x01, 0x01, 0x00, 0x12, 0x2d, 0x24, 0xf2, 0x24, 0x96, 0xa4, 0xff, 0x98, 0x29, 0xf9,
    0x21, 0xbe, 0x9d, 0xaa, 0x9e, 0x4d, 0x01, 0x17,
];

/// Instruction d'un message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Instruction {
    Read = 1,
    Write = 2,
    Answer = 3,
}

/// Identifiants de valeur utilisés par l'application (voir le tableau du CLAUDE.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ValueId {
    Handshake = 1,
    DeviceState = 6,
    Pulse = 8,
    Distance = 15,
    Energy = 16,
    DeviceName = 20,
    Article = 22,
    DeviceType = 24,
    DeviceId = 25,
    InclineSet = 30,
    InclineGet = 31,
    InclineMin = 32,
    InclineMax = 33,
    SpeedSet = 34,
    SpeedGet = 35,
    SpeedMin = 36,
    SpeedMax = 37,
    Online = 38,
}

impl ValueId {
    /// Retrouve l'identifiant d'un message reçu ; `None` si inconnu de l'application.
    pub fn from_u16(v: u16) -> Option<ValueId> {
        use ValueId::*;
        Some(match v {
            1 => Handshake,
            6 => DeviceState,
            8 => Pulse,
            15 => Distance,
            16 => Energy,
            20 => DeviceName,
            22 => Article,
            24 => DeviceType,
            25 => DeviceId,
            30 => InclineSet,
            31 => InclineGet,
            32 => InclineMin,
            33 => InclineMax,
            34 => SpeedSet,
            35 => SpeedGet,
            36 => SpeedMin,
            37 => SpeedMax,
            38 => Online,
            _ => return None,
        })
    }
}

/// Message décodé (payload sans trame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Identifiant brut (peut ne pas correspondre à un `ValueId`).
    pub value_id: u16,
    /// Octet d'instruction brut (3 = réponse).
    pub instruction: u8,
    pub data: Vec<u8>,
}

impl Message {
    /// Données interprétées en u16 big-endian (2 octets), sinon `None`.
    pub fn as_u16(&self) -> Option<u16> {
        match self.data.as_slice() {
            [hi, lo] => Some(u16::from_be_bytes([*hi, *lo])),
            _ => None,
        }
    }

    /// Données sur 1 octet, sinon `None`.
    pub fn as_u8(&self) -> Option<u8> {
        match self.data.as_slice() {
            [b] => Some(*b),
            _ => None,
        }
    }

    /// Données ASCII (DeviceName, DeviceId, Article…), sinon `None`.
    ///
    /// Choix : les octets nuls de fin (bourrage éventuel) sont retirés, puis
    /// seuls les caractères ASCII imprimables (0x20 à 0x7E) sont acceptés.
    /// Données vides (après retrait des nuls) ou contenant un autre octet → `None`.
    /// Aucun espace n'est retiré : `RUN 7` reste tel quel.
    pub fn as_text(&self) -> Option<String> {
        let end = self
            .data
            .iter()
            .rposition(|&b| b != 0)
            .map_or(0, |i| i + 1);
        let bytes = &self.data[..end];
        if bytes.is_empty() || !bytes.iter().all(|b| (0x20..=0x7e).contains(b)) {
            return None;
        }
        // Que de l'ASCII imprimable : la conversion ne peut pas échouer.
        Some(bytes.iter().map(|&b| b as char).collect())
    }
}

/// Erreur de décodage d'une trame reçue (la trame est ignorée, le flux continue).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("CRC invalide")]
    BadCrc,
    #[error("échappement invalide")]
    BadEscape,
    #[error("trame trop courte")]
    TooShort,
    #[error("longueur incohérente")]
    BadLength,
}

/// Table du CRC16 réfléchi (poly 0x8408), calculée à la compilation.
const CRC_TABLE: [u16; 256] = build_crc_table();

const fn build_crc_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc: u16 = 0;
        let mut d: u16 = i as u16;
        let mut bit = 0;
        while bit < 8 {
            crc = if (d ^ crc) & 1 != 0 {
                (crc >> 1) ^ 0x8408
            } else {
                crc >> 1
            };
            d >>= 1;
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// CRC16 Kermit/CCITT réfléchi (poly 0x8408, init 0, sans XOR final).
pub fn crc16(data: &[u8]) -> u16 {
    data.iter().fold(0u16, |c, &b| {
        (c >> 8) ^ CRC_TABLE[((c ^ b as u16) & 0xff) as usize]
    })
}

/// Vrai si l'octet doit être échappé dans une trame.
fn needs_escape(b: u8) -> bool {
    matches!(b, STX | ETX | DLE)
}

/// Ajoute un octet à la trame, échappé si nécessaire.
fn push_escaped(out: &mut Vec<u8>, b: u8) {
    if needs_escape(b) {
        out.push(DLE);
        out.push(b ^ ESC_XOR);
    } else {
        out.push(b);
    }
}

/// Encapsule un payload : 0x02, payload échappé, 0x03, CRC (poids fort d'abord) échappé.
pub fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() * 2 + 6);
    out.push(STX);
    for &b in payload {
        push_escaped(&mut out, b);
    }
    out.push(ETX);
    for b in crc16(payload).to_be_bytes() {
        push_escaped(&mut out, b);
    }
    out
}

/// Construit le payload d'un message (en-tête + données).
fn payload(id: u16, instruction: Instruction, data: &[u8]) -> Vec<u8> {
    let mut p = Vec::with_capacity(HEADER_LEN + data.len());
    p.extend_from_slice(&id.to_be_bytes());
    p.push(instruction as u8);
    p.extend_from_slice(&(data.len() as u16).to_be_bytes());
    p.extend_from_slice(data);
    p
}

/// Trame complète du handshake obligatoire (`… 03 86 77`).
pub fn handshake_frame() -> Vec<u8> {
    encode_frame(&HANDSHAKE_PAYLOAD)
}

/// Données du handshake (les 18 octets que le tapis renvoie à l'identique).
pub fn handshake_data() -> &'static [u8] {
    &HANDSHAKE_PAYLOAD[HEADER_LEN..]
}

/// Trame de réponse (instruction 3), telle que le tapis l'envoie. Sert au
/// simulateur et aux tests ; l'application n'en envoie jamais.
pub fn answer_frame(id: u16, data: &[u8]) -> Vec<u8> {
    encode_frame(&payload(id, Instruction::Answer, data))
}

/// Trame de lecture d'une valeur.
pub fn read_frame(id: ValueId) -> Vec<u8> {
    encode_frame(&payload(id as u16, Instruction::Read, &[]))
}

/// Trame d'écriture d'une valeur sur 1 octet (Online).
pub fn write_u8_frame(id: ValueId, value: u8) -> Vec<u8> {
    encode_frame(&payload(id as u16, Instruction::Write, &[value]))
}

/// Trame d'écriture d'une valeur sur 2 octets big-endian (SpeedSet, InclineSet).
pub fn write_u16_frame(id: ValueId, value: u16) -> Vec<u8> {
    encode_frame(&payload(id as u16, Instruction::Write, &value.to_be_bytes()))
}

/// Interprète un payload désechappé dont le CRC a déjà été vérifié.
fn parse_payload(p: &[u8]) -> Result<Message, DecodeError> {
    if p.len() < HEADER_LEN {
        return Err(DecodeError::TooShort);
    }
    let value_id = u16::from_be_bytes([p[0], p[1]]);
    let instruction = p[2];
    let len = u16::from_be_bytes([p[3], p[4]]) as usize;
    if p.len() != HEADER_LEN + len {
        return Err(DecodeError::BadLength);
    }
    Ok(Message {
        value_id,
        instruction,
        data: p[HEADER_LEN..].to_vec(),
    })
}

/// Où en est le décodeur dans la trame courante.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Hors trame : on attend un 0x02, tout le reste est ignoré.
    #[default]
    Idle,
    /// Dans le payload.
    Payload,
    /// Après le 0x03 : réception des 2 octets de CRC.
    Crc,
}

/// Décodeur à flux : les notifications BLE coupent les trames n'importe où.
///
/// Règles :
/// - un 0x02 non échappé (re)commence une trame, même au milieu d'une autre
///   (la trame interrompue est abandonnée sans erreur, comme la référence Python) ;
/// - hors trame, tout octet autre que 0x02 est ignoré ;
/// - 0x10 échappe l'octet suivant, dans le payload comme dans le CRC ; un octet
///   échappé qui ne redonne pas 0x02, 0x03 ou 0x10 → `BadEscape` et retour hors trame ;
/// - le premier 0x03 non échappé termine le payload ; viennent ensuite 2 octets de CRC
///   (un 0x03 brut y est accepté comme valeur, par tolérance) ;
/// - ordre des vérifications : CRC (`BadCrc`), taille minimale (`TooShort`),
///   cohérence de la longueur (`BadLength`) ;
/// - un payload dépassant `MAX_PAYLOAD` sans 0x03 → `BadLength` et retour hors trame.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    state: State,
    /// Le dernier octet reçu était un 0x10 d'échappement.
    escaped: bool,
    /// Payload désechappé de la trame courante.
    payload: Vec<u8>,
    /// Octets de CRC désechappés reçus (0 à 2).
    crc: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute des octets reçus et rend chaque trame complétée par ces octets.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Message, DecodeError>> {
        let mut out = Vec::new();
        for &raw in bytes {
            if let Some(result) = self.feed(raw) {
                out.push(result);
            }
        }
        out
    }

    /// Repart hors trame, tampons vidés.
    fn reset(&mut self) {
        self.state = State::Idle;
        self.escaped = false;
        self.payload.clear();
        self.crc.clear();
    }

    /// Démarre une nouvelle trame (après un 0x02 non échappé).
    fn start(&mut self) {
        self.reset();
        self.state = State::Payload;
    }

    /// Traite un octet brut ; rend un résultat quand une trame se termine.
    fn feed(&mut self, raw: u8) -> Option<Result<Message, DecodeError>> {
        // Un 0x02 brut n'apparaît jamais dans une trame valide (il serait échappé) :
        // c'est toujours un début de trame, même juste après un 0x10.
        if raw == STX {
            self.start();
            return None;
        }
        if self.state == State::Idle {
            return None;
        }

        let byte = if self.escaped {
            self.escaped = false;
            let b = raw ^ ESC_XOR;
            if !needs_escape(b) {
                self.reset();
                return Some(Err(DecodeError::BadEscape));
            }
            b
        } else if raw == DLE {
            self.escaped = true;
            return None;
        } else if raw == ETX && self.state == State::Payload {
            self.state = State::Crc;
            return None;
        } else {
            raw
        };

        match self.state {
            State::Payload => {
                if self.payload.len() >= MAX_PAYLOAD {
                    self.reset();
                    return Some(Err(DecodeError::BadLength));
                }
                self.payload.push(byte);
                None
            }
            State::Crc => {
                self.crc.push(byte);
                if self.crc.len() < 2 {
                    return None;
                }
                let received = u16::from_be_bytes([self.crc[0], self.crc[1]]);
                let result = if crc16(&self.payload) != received {
                    Err(DecodeError::BadCrc)
                } else {
                    parse_payload(&self.payload)
                };
                self.reset();
                Some(result)
            }
            State::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convertit « 02 00 23 … » en octets.
    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect()
    }

    /// Décode un flux complet et exige exactement un message valide.
    fn decode_one(bytes: &[u8]) -> Message {
        let mut d = FrameDecoder::new();
        let mut res = d.push(bytes);
        assert_eq!(res.len(), 1, "un seul résultat attendu : {res:?}");
        res.remove(0).expect("trame valide attendue")
    }

    #[test]
    fn crc_du_handshake() {
        assert_eq!(crc16(&HANDSHAKE_PAYLOAD), 0x8677);
        assert_eq!(crc16(&[]), 0);
    }

    #[test]
    fn trame_handshake() {
        assert_eq!(
            handshake_frame(),
            hex("02 00 01 01 00 12 2d 24 f2 24 96 a4 ff 98 29 f9 21 be 9d aa 9e 4d 01 17 03 86 77")
        );
    }

    #[test]
    fn trame_lecture_speed_get() {
        assert_eq!(read_frame(ValueId::SpeedGet), hex("02 00 23 01 00 00 03 f0 42"));
    }

    #[test]
    fn trame_ecriture_speed_set_25() {
        assert_eq!(
            write_u16_frame(ValueId::SpeedSet, 25),
            hex("02 00 22 10 22 00 10 22 00 19 03 a3 46")
        );
    }

    #[test]
    fn trame_ecriture_incline_set_10() {
        assert_eq!(
            write_u16_frame(ValueId::InclineSet, 10),
            hex("02 00 1e 10 22 00 10 22 00 0a 03 77 78")
        );
    }

    #[test]
    fn trames_online_demarrer_arreter() {
        assert_eq!(
            write_u8_frame(ValueId::Online, 0),
            hex("02 00 26 10 22 00 01 00 03 7b a7")
        );
        assert_eq!(
            write_u8_frame(ValueId::Online, 1),
            hex("02 00 26 10 22 00 01 01 03 6a 2e")
        );
    }

    #[test]
    fn decode_reponse_device_type() {
        let m = decode_one(&hex("02 00 18 10 23 00 01 54 03 c6 d4"));
        assert_eq!(m.value_id, 24);
        assert_eq!(ValueId::from_u16(m.value_id), Some(ValueId::DeviceType));
        assert_eq!(m.instruction, Instruction::Answer as u8);
        assert_eq!(m.as_text().as_deref(), Some("T"));
        assert_eq!(m.as_u8(), Some(b'T'));
        assert_eq!(m.as_u16(), None);
    }

    #[test]
    fn decode_reponse_avec_crc_echappe() {
        // SpeedGet = 19 (1,9 km/h) : CRC 03 73, transmis 10 23 73.
        let m = decode_one(&hex("02 00 23 10 23 00 10 22 00 13 03 10 23 73"));
        assert_eq!(ValueId::from_u16(m.value_id), Some(ValueId::SpeedGet));
        assert_eq!(m.instruction, 3);
        assert_eq!(m.as_u16(), Some(19));
        assert_eq!(m.as_u8(), None);
    }

    #[test]
    fn encode_echappe_aussi_le_crc() {
        // Même réponse que ci-dessus, reconstruite : le CRC 03 73 doit sortir échappé.
        let p = payload(35, Instruction::Answer, &19u16.to_be_bytes());
        assert_eq!(crc16(&p), 0x0373);
        assert_eq!(encode_frame(&p), hex("02 00 23 10 23 00 10 22 00 13 03 10 23 73"));
    }

    #[test]
    fn trame_decoupee_en_morceaux_arbitraires() {
        let frame = hex("02 00 23 10 23 00 10 22 00 13 03 10 23 73");
        // Toutes les coupures possibles en deux morceaux, dont au milieu d'un échappement.
        for cut in 0..=frame.len() {
            let mut d = FrameDecoder::new();
            let mut res = d.push(&frame[..cut]);
            res.extend(d.push(&frame[cut..]));
            assert_eq!(res.len(), 1, "coupure en {cut}");
            assert_eq!(res[0].as_ref().unwrap().as_u16(), Some(19));
        }
        // Octet par octet : rien avant le dernier octet.
        let mut d = FrameDecoder::new();
        for (i, b) in frame.iter().enumerate() {
            let res = d.push(&[*b]);
            if i + 1 < frame.len() {
                assert!(res.is_empty(), "résultat prématuré à l'octet {i}");
            } else {
                assert_eq!(res.len(), 1);
                assert_eq!(res[0].as_ref().unwrap().as_u16(), Some(19));
            }
        }
        // Morceaux de 3 octets (comme des notifications BLE irrégulières).
        let hs = handshake_frame();
        let mut d = FrameDecoder::new();
        let res: Vec<_> = hs.chunks(3).flat_map(|c| d.push(c)).collect();
        assert_eq!(res.len(), 1);
        let m = res[0].as_ref().unwrap();
        assert_eq!(m.value_id, 1);
        assert_eq!(m.instruction, 1);
        assert_eq!(m.data, HANDSHAKE_PAYLOAD[5..].to_vec());
    }

    #[test]
    fn plusieurs_trames_dans_un_seul_push() {
        let mut flux = hex("ff 00"); // déchets hors trame, ignorés
        flux.extend(hex("02 00 18 10 23 00 01 54 03 c6 d4"));
        flux.extend(hex("02 00 23 10 23 00 10 22 00 13 03 10 23 73"));
        flux.extend(hex("aa 55"));
        flux.extend(write_u8_frame(ValueId::Online, 1));
        let res = FrameDecoder::new().push(&flux);
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].as_ref().unwrap().as_text().as_deref(), Some("T"));
        assert_eq!(res[1].as_ref().unwrap().as_u16(), Some(19));
        let online = res[2].as_ref().unwrap();
        assert_eq!(ValueId::from_u16(online.value_id), Some(ValueId::Online));
        assert_eq!(online.instruction, Instruction::Write as u8);
        assert_eq!(online.as_u8(), Some(1));
    }

    #[test]
    fn crc_faux_puis_flux_normal() {
        let mut d = FrameDecoder::new();
        // Dernier octet de CRC altéré (d4 → d5).
        assert_eq!(d.push(&hex("02 00 18 10 23 00 01 54 03 c6 d5")), vec![Err(DecodeError::BadCrc)]);
        let res = d.push(&hex("02 00 23 10 23 00 10 22 00 13 03 10 23 73"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].as_ref().unwrap().as_u16(), Some(19));
    }

    #[test]
    fn erreurs_de_structure() {
        // Payload trop court (CRC correct).
        let court = [0x00, 0x23, 0x03];
        assert_eq!(FrameDecoder::new().push(&encode_frame(&court)), vec![Err(DecodeError::TooShort)]);
        // Longueur annoncée 2, 1 seul octet de données.
        let incoherent = [0x00, 0x23, 0x03, 0x00, 0x02, 0x13];
        assert_eq!(
            FrameDecoder::new().push(&encode_frame(&incoherent)),
            vec![Err(DecodeError::BadLength)]
        );
        // Échappement invalide (0x10 0x41 → 0x61), puis le flux repart.
        let mut d = FrameDecoder::new();
        let mut flux = hex("02 00 10 41 00");
        flux.extend(read_frame(ValueId::SpeedGet));
        let res = d.push(&flux);
        assert_eq!(res.len(), 2);
        assert_eq!(res[0], Err(DecodeError::BadEscape));
        assert_eq!(res[1].as_ref().unwrap().value_id, 35);
        // Trame interrompue par un nouveau 0x02 : abandonnée sans erreur.
        let mut flux = hex("02 00 23 01");
        flux.extend(read_frame(ValueId::SpeedMax));
        let res = FrameDecoder::new().push(&flux);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].as_ref().unwrap().value_id, 37);
    }

    #[test]
    fn aller_retour_avec_octets_speciaux() {
        let valeurs: [u16; 8] = [0x0002, 0x0003, 0x0010, 0x0203, 0x1010, 0x0302, 0x1002, 0xffff];
        for v in valeurs {
            let m = decode_one(&write_u16_frame(ValueId::SpeedSet, v));
            assert_eq!(m.value_id, ValueId::SpeedSet as u16);
            assert_eq!(m.instruction, 2);
            assert_eq!(m.as_u16(), Some(v), "valeur {v:#06x}");
        }
        for v in [0x02u8, 0x03, 0x10, 0x00, 0xff] {
            let m = decode_one(&write_u8_frame(ValueId::Online, v));
            assert_eq!(m.as_u8(), Some(v));
        }
        // Tous les octets possibles dans les données, identifiant et longueur spéciaux.
        let data: Vec<u8> = (0..=255u8).collect();
        let p = payload(0x0210, Instruction::Answer, &data);
        let m = decode_one(&encode_frame(&p));
        assert_eq!(m.value_id, 0x0210);
        assert_eq!(m.data, data);
        // Valeurs dont le CRC contient un octet spécial : vérifie l'échappement du CRC.
        let mut crc_special = 0;
        for v in 0..=u16::MAX {
            let p = payload(35, Instruction::Answer, &v.to_be_bytes());
            if crc16(&p).to_be_bytes().iter().any(|b| needs_escape(*b)) {
                crc_special += 1;
                assert_eq!(decode_one(&encode_frame(&p)).as_u16(), Some(v));
            }
        }
        assert!(crc_special > 0);
    }

    #[test]
    fn texte_et_identifiants() {
        let msg = |data: &[u8]| Message { value_id: 20, instruction: 3, data: data.to_vec() };
        assert_eq!(msg(b"RUN 7").as_text().as_deref(), Some("RUN 7"));
        assert_eq!(msg(b"STB1T0158\0\0").as_text().as_deref(), Some("STB1T0158"));
        assert_eq!(msg(&[0x06, 0xac, 0xfc, 0x01]).as_text(), None);
        assert_eq!(msg(&[]).as_text(), None);
        assert_eq!(ValueId::from_u16(38), Some(ValueId::Online));
        assert_eq!(ValueId::from_u16(2), None);
        assert_eq!(ValueId::from_u16(17), None);
        for id in [ValueId::Handshake, ValueId::Article, ValueId::SpeedMax, ValueId::DeviceId] {
            assert_eq!(ValueId::from_u16(id as u16), Some(id));
        }
    }
}
