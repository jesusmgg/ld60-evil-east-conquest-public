use crate::constants::HISTORY_SNAPSHOTS_MAGIC;
use crate::game_data::game_data::{is_readable_save_version, DATA_VERSION};
use crate::history_manager::service::{read_u16_at, read_u32_at};

pub const CHECKPOINT_NAME: &str = "Checkpoint";
pub const NEW_GAME_NAME: &str = "New game";

const MIN_SNAPSHOT_BYTES: usize = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotKind {
    Manual,
    Checkpoint,
    NewGame,
}

impl SnapshotKind {
    fn to_byte(self) -> u8 {
        match self {
            SnapshotKind::Manual => 0,
            SnapshotKind::Checkpoint => 1,
            SnapshotKind::NewGame => 2,
        }
    }

    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(SnapshotKind::Manual),
            1 => Some(SnapshotKind::Checkpoint),
            2 => Some(SnapshotKind::NewGame),
            _ => None,
        }
    }
}

pub struct Snapshot {
    pub kind: SnapshotKind,
    pub name: String,
    pub turn: u32,
    pub picture: Vec<u8>,
}

pub fn encode_snapshots(snapshots: &[Snapshot]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&HISTORY_SNAPSHOTS_MAGIC);
    output.extend_from_slice(&(DATA_VERSION as u32).to_le_bytes());
    output.extend_from_slice(&(snapshots.len() as u32).to_le_bytes());
    for snapshot in snapshots {
        output.push(snapshot.kind.to_byte());
        output.extend_from_slice(&(snapshot.name.len() as u16).to_le_bytes());
        output.extend_from_slice(snapshot.name.as_bytes());
        output.extend_from_slice(&snapshot.turn.to_le_bytes());
        output.extend_from_slice(&(snapshot.picture.len() as u32).to_le_bytes());
        output.extend_from_slice(&snapshot.picture);
    }
    output
}

pub fn decode_snapshots(bytes: &[u8]) -> Option<Vec<Snapshot>> {
    if bytes.len() < 8 || &bytes[0..8] != &HISTORY_SNAPSHOTS_MAGIC {
        return None;
    }
    let mut offset = 8;
    let format_version = read_u32_at(bytes, &mut offset)?;
    if !is_readable_save_version(format_version) {
        return None;
    }
    let count = read_u32_at(bytes, &mut offset)? as usize;
    if count > (bytes.len() - offset) / MIN_SNAPSHOT_BYTES {
        return None;
    }
    let mut snapshots = Vec::with_capacity(count);
    for _ in 0..count {
        if offset >= bytes.len() {
            return None;
        }
        let kind = SnapshotKind::from_byte(bytes[offset])?;
        offset += 1;
        let name_length = read_u16_at(bytes, &mut offset)? as usize;
        if name_length > bytes.len() - offset {
            return None;
        }
        let name = String::from_utf8(bytes[offset..offset + name_length].to_vec()).ok()?;
        offset += name_length;
        let turn = read_u32_at(bytes, &mut offset)?;
        let picture_length = read_u32_at(bytes, &mut offset)? as usize;
        if picture_length > bytes.len() - offset {
            return None;
        }
        let picture = bytes[offset..offset + picture_length].to_vec();
        offset += picture_length;
        snapshots.push(Snapshot {
            kind,
            name,
            turn,
            picture,
        });
    }
    if offset != bytes.len() {
        return None;
    }
    Some(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_round_trip() {
        let snapshots = vec![
            Snapshot {
                kind: SnapshotKind::NewGame,
                name: String::from(NEW_GAME_NAME),
                turn: 0,
                picture: vec![1, 2, 3],
            },
            Snapshot {
                kind: SnapshotKind::Manual,
                name: String::from("Before the bridge"),
                turn: 812,
                picture: vec![9; 40],
            },
        ];
        let bytes = encode_snapshots(&snapshots);
        let decoded = decode_snapshots(&bytes).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].kind, SnapshotKind::NewGame);
        assert_eq!(decoded[0].name, NEW_GAME_NAME);
        assert_eq!(decoded[0].picture, vec![1, 2, 3]);
        assert_eq!(decoded[1].kind, SnapshotKind::Manual);
        assert_eq!(decoded[1].name, "Before the bridge");
        assert_eq!(decoded[1].turn, 812);
        assert_eq!(decoded[1].picture, vec![9; 40]);

        assert!(decode_snapshots(&bytes[..bytes.len() - 1]).is_none());
        assert!(decode_snapshots(&bytes[..20]).is_none());
        assert!(decode_snapshots(&[]).is_none());
    }
}
