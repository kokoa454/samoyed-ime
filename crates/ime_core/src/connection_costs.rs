use crate::kanji_converter::ConnectionCostProvider;


const MAGIC: &[u8; 8] = b"SMYCST01"; // Mozc形式
const HEADER_SIZE: usize = 12; // バイナリヘッダのサイズ
const COST_SIZE: usize = 2; // 1コストあたりのバイト数


/// 連接コストバイナリの読み込みエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionCostError {
    TruncatedHeader, // ヘッダーを読み取るだけのデータがない
    InvalidMagic, // magicが想定した値と一致しない
    InvalidDimension, // 行列の次元が不正
    SizeOverflow, // 必要なサイズの計算でオーバーフローした
    InvalidLength, // バイナリの実際の長さと期待する長さが一致しない
}

/// 連接コスト行列への読み取り専用アクセス
#[derive(Debug)]
pub struct ConnectionCosts<'a> {
    data: &'a [u8], // 連接コストのバイナリデータ（コピーせず参照を保持）
    dimension: usize, // 行列の次元
}

impl<'a> ConnectionCosts<'a> {
    /// バイナリから連接コストを読み込む。
    /// 
    /// 引数:
    /// * `data`: 連接コストのバイナリデータ（コピーせず参照を保持）
    /// 
    /// 戻り値:
    /// * `Ok(ConnectionCosts)`: 成功
    /// * `Err(ConnectionCostError)`: エラー発生時
    pub fn from_bytes(
        data: &'a [u8],
    ) -> Result<Self, ConnectionCostError> {
        if data.len() < HEADER_SIZE {
            return Err(ConnectionCostError::TruncatedHeader);
        }

        if &data[..MAGIC.len()] != MAGIC {
            return Err(ConnectionCostError::InvalidMagic);
        }

        let dimension_bytes: [u8; 4] = data[8..12]
            .try_into()
            .map_err(|_| ConnectionCostError::TruncatedHeader)?;

        let dimension = u32::from_le_bytes(dimension_bytes) as usize;

        if dimension == 0 {
            return Err(ConnectionCostError::InvalidDimension);
        }

        // dimension * dimension * COST_SIZE + HEADER_SIZE の計算でオーバーフローしないことを確認する
        let entry_count = dimension
            .checked_mul(dimension)
            .ok_or(ConnectionCostError::SizeOverflow)?;

        let cost_bytes = entry_count
            .checked_mul(COST_SIZE)
            .ok_or(ConnectionCostError::SizeOverflow)?;

        let expected_length = HEADER_SIZE
            .checked_add(cost_bytes)
            .ok_or(ConnectionCostError::SizeOverflow)?;

        if data.len() != expected_length {
            return Err(ConnectionCostError::InvalidLength);
        }

        Ok(Self { data, dimension })
    }

    /// ビルド時に生成された連接コストを読み込む。
    /// 
    /// 戻り値:
    /// * `Ok(ConnectionCosts)`: 成功
    /// * `Err(ConnectionCostError)`: エラー発生時
    pub fn embedded() -> Result<ConnectionCosts<'static>, ConnectionCostError> {
        let data: &'static [u8] = include_bytes!(concat!(
            env!("OUT_DIR"),
            "/connection_costs.bin"
        ));

        ConnectionCosts::from_bytes(data)
    }

    /// 行列の次元を返す。
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// 右文脈IDと左文脈IDを指定してコストを取得する。
    /// 行列は [right_id][left_id] の順序で格納されている。
    /// 
    /// 引数:
    /// * `right_id`: 右文脈ID
    /// * `left_id`: 左文脈ID
    /// 
    /// 戻り値:
    /// * `Some(i32)`: コスト
    /// * `None`: IDが範囲外の場合
    fn cost(
        &self,
        right_id: u16,
        left_id: u16,
    ) -> Option<i32> {
        let right = usize::from(right_id);
        let left = usize::from(left_id);

        if right >= self.dimension || left >= self.dimension {
            return None;
        }

        let index = right
            .checked_mul(self.dimension)?
            .checked_add(left)?;

        let offset = HEADER_SIZE
            .checked_add(index.checked_mul(COST_SIZE)?)?;

        let end = offset.checked_add(COST_SIZE)?;
        let bytes = self.data.get(offset..end)?;

        let value = u16::from_le_bytes([bytes[0], bytes[1]]);

        Some(i32::from(value))
    }
}

impl ConnectionCostProvider for ConnectionCosts<'_> {
    /// 文頭から最初の単語へ接続するコスト。
    /// 
    /// 文頭・文末を表すIDは0。
    /// 
    /// 引数:
    /// * `left_id`: 左文脈ID
    /// 
    /// 戻り値:
    /// * `Some(i32)`: コスト
    /// * `None`: IDが範囲外の場合
    fn start_cost(&self, left_id: u16) -> Option<i32> {
        self.cost(0, left_id)
    }

    /// 直前の単語から次の単語へ接続するコスト。
    /// 
    /// 引数:
    /// * `previous_right_id`: 直前の単語の右文脈ID
    /// * `next_left_id`: 次の単語の左文脈ID
    /// 
    /// 戻り値:
    /// * `Some(i32)`: コスト
    /// * `None`: IDが範囲外の場合
    fn transition_cost(
        &self,
        previous_right_id: u16,
        next_left_id: u16,
    ) -> Option<i32> {
        self.cost(previous_right_id, next_left_id)
    }

    /// 最後の単語から文末へ接続するコスト。
    /// 
    /// 引数:
    /// * `right_id`: 右文脈ID
    /// 
    /// 戻り値:
    /// * `Some(i32)`: コスト
    /// * `None`: IDが範囲外の場合
    fn end_cost(&self, right_id: u16) -> Option<i32> {
        self.cost(right_id, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用の連接コストバイナリを作る。
    /// 
    /// 引数:
    /// * `dimension`: 行列の次元
    /// * `costs`: 連接コストの配列（right_id * dimension + left_id の順序）
    /// 
    /// 戻り値:
    /// * `Vec<u8>`: バイナリデータ
    fn make_binary(dimension: u32, costs: &[u16]) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&dimension.to_le_bytes());

        for cost in costs {
            bytes.extend_from_slice(&cost.to_le_bytes());
        }

        bytes
    }

    #[test]
    fn reads_costs_in_right_left_order() {
        // 2x2の行列:
        //
        //             left=0  left=1
        // right=0        0       7
        // right=1       11      13
        //
        // フラット化すると [0, 7, 11, 13]
        let bytes = make_binary(2, &[0, 7, 11, 13]);
        let costs = ConnectionCosts::from_bytes(&bytes).unwrap();

        assert_eq!(costs.dimension(), 2);

        assert_eq!(costs.start_cost(0), Some(0));
        assert_eq!(costs.start_cost(1), Some(7));

        assert_eq!(costs.transition_cost(1, 0), Some(11));
        assert_eq!(costs.transition_cost(1, 1), Some(13));

        assert_eq!(costs.end_cost(0), Some(0));
        assert_eq!(costs.end_cost(1), Some(11));
    }

    #[test]
    fn rejects_invalid_magic() {
        let mut bytes = make_binary(1, &[0]);
        bytes[0] = b'X';

        assert_eq!(
            ConnectionCosts::from_bytes(&bytes).unwrap_err(),
            ConnectionCostError::InvalidMagic
        );
    }

    #[test]
    fn rejects_truncated_header() {
        assert_eq!(
            ConnectionCosts::from_bytes(b"SMYCST01").unwrap_err(),
            ConnectionCostError::TruncatedHeader
        );
    }

    #[test]
    fn rejects_zero_dimension() {
        let bytes = make_binary(0, &[]);

        assert_eq!(
            ConnectionCosts::from_bytes(&bytes).unwrap_err(),
            ConnectionCostError::InvalidDimension
        );
    }

    #[test]
    fn rejects_wrong_cost_count() {
        // 2x2なら4個必要だが、3個しかない。
        let bytes = make_binary(2, &[0, 1, 2]);

        assert_eq!(
            ConnectionCosts::from_bytes(&bytes).unwrap_err(),
            ConnectionCostError::InvalidLength
        );
    }

    #[test]
    fn embedded_matrix_can_be_loaded() {
        let costs = ConnectionCosts::embedded().unwrap();

        assert!(costs.dimension() > 0);

        // BOS/EOS ID 0 が有効なIDとして参照できることを確認
        assert!(costs.start_cost(0).is_some());
        assert!(costs.end_cost(0).is_some());
    }

    /// 期待するサイズより余分なバイトがある場合は拒否する。
    #[test]
    fn rejects_trailing_bytes() {
        let mut bytes = make_binary(1, &[42]);
        bytes.push(0);

        assert_eq!(
            ConnectionCosts::from_bytes(&bytes).unwrap_err(),
            ConnectionCostError::InvalidLength
        );
    }

    /// 行列のサイズ計算がオーバーフローする場合は拒否する。
    #[test]
    fn rejects_size_overflow() {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());

        assert_eq!(
            ConnectionCosts::from_bytes(&bytes).unwrap_err(),
            ConnectionCostError::SizeOverflow
        );
    }

    /// u16の上位ビットを含むコスト値を正しく読み取る。
    #[test]
    fn preserves_unsigned_cost_values() {
        let bytes = make_binary(2, &[0, 32_768, 65_535, 42]);
        let costs = ConnectionCosts::from_bytes(&bytes).unwrap();

        assert_eq!(costs.start_cost(1), Some(32_768));
        assert_eq!(costs.transition_cost(1, 0), Some(65_535));
    }

    /// 行列の次元を超える文脈IDは拒否する。
    #[test]
    fn rejects_out_of_range_context_ids() {
        let bytes = make_binary(2, &[0, 7, 11, 13]);
        let costs = ConnectionCosts::from_bytes(&bytes).unwrap();

        assert_eq!(costs.transition_cost(2, 0), None);
        assert_eq!(costs.transition_cost(0, 2), None);
        assert_eq!(costs.start_cost(2), None);
        assert_eq!(costs.end_cost(2), None);
    }
}