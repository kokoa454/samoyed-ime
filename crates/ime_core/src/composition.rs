/// 未確定文字列とカーソル位置を管理する構造体
pub struct Composition {
    content_units: Vec<char>, // 未確定文字列
    cursor_pos: usize, // 未確定文字列のカーソル位置
}

impl Composition {
    /// コンストラクタ
    pub fn new() -> Self {
        Self {
            content_units: Vec::new(),
            cursor_pos: 0,
        }
    }

    /// 未確定文字列を取得する
    pub fn get_content_units(&self) -> &[char] {
        &self.content_units
    }

    /// 文字をカーソル位置に挿入する
    pub fn insert(&mut self, input: char) {
        self.content_units.insert(self.cursor_pos, input);
        self.cursor_pos += 1;
    }

    /// カーソルの直前の文字を削除する
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.content_units.remove(self.cursor_pos);
        }
    }

    /// カーソルの直後の文字を削除する
    pub fn delete(&mut self) {
        if self.cursor_pos < self.content_units.len() {
            self.content_units.remove(self.cursor_pos);
        }
    }

    /// 未確定文字列をクリアする
    pub fn clear(&mut self) {
        self.content_units.clear();
        self.cursor_pos = 0;
    }

    /// 未確定文字列の長さを取得する
    pub fn get_content_units_len(&self) -> usize {
        self.content_units.len()
    }

    /// カーソルを右に移動させる
    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.content_units.len() {
            self.cursor_pos += 1;
        }
    }

    /// カーソルを左に移動させる
    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    /// カーソルを先頭に移動させる
    pub fn move_cursor_to_head(&mut self) {
        self.cursor_pos = 0;
    }

    /// カーソルを末尾に移動させる
    pub fn move_cursor_to_tail(&mut self) {
        self.cursor_pos = self.content_units.len();
    }

    /// カーソル位置を取得する
    pub fn get_cursor_pos(&self) -> usize {
        self.cursor_pos
    }
}


/// テスト
#[cfg(test)]
mod tests {
    use super::*;

    fn composition_to_string(composition: &Composition) -> String {
        composition.get_content_units().iter().collect()
    }

    // ============================================================
    // 初期状態
    // ============================================================

    #[test]
    fn test_new() {
        let composition = Composition::new();

        assert_eq!(composition_to_string(&composition), "");
        assert_eq!(composition.get_content_units_len(), 0);
        assert_eq!(composition.get_cursor_pos(), 0);
    }

    // ============================================================
    // 挿入
    // ============================================================

    #[test]
    fn test_insert() {
        let mut composition = Composition::new();

        composition.insert('あ');

        assert_eq!(composition_to_string(&composition), "あ");
        assert_eq!(composition.get_content_units_len(), 1);
        assert_eq!(composition.get_cursor_pos(), 1);
    }

    #[test]
    fn test_insert_multiple() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        assert_eq!(composition_to_string(&composition), "あいう");
        assert_eq!(composition.get_content_units_len(), 3);
        assert_eq!(composition.get_cursor_pos(), 3);
    }

    #[test]
    fn test_insert_at_middle() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('う');

        composition.move_cursor_left();
        composition.insert('い');

        assert_eq!(composition_to_string(&composition), "あいう");
        assert_eq!(composition.get_cursor_pos(), 2);
    }

    // ============================================================
    // Backspace
    // ============================================================

    #[test]
    fn test_backspace() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.backspace();

        assert_eq!(composition_to_string(&composition), "あい");
        assert_eq!(composition.get_cursor_pos(), 2);
    }

    #[test]
    fn test_backspace_at_head_does_nothing() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.move_cursor_to_head();

        composition.backspace();

        assert_eq!(composition_to_string(&composition), "あい");
        assert_eq!(composition.get_cursor_pos(), 0);
    }

    #[test]
    fn test_backspace_in_middle() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_left();
        composition.backspace();

        assert_eq!(composition_to_string(&composition), "あう");
        assert_eq!(composition.get_cursor_pos(), 1);
    }

    // ============================================================
    // Delete
    // ============================================================

    #[test]
    fn test_delete() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_to_head();
        composition.delete();

        assert_eq!(composition_to_string(&composition), "いう");
        assert_eq!(composition.get_cursor_pos(), 0);
    }

    #[test]
    fn test_delete_at_tail_does_nothing() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');

        composition.move_cursor_to_tail();
        composition.delete();

        assert_eq!(composition_to_string(&composition), "あい");
        assert_eq!(composition.get_cursor_pos(), 2);
    }

    #[test]
    fn test_delete_in_middle() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_to_head();
        composition.move_cursor_right();
        composition.delete();

        assert_eq!(composition_to_string(&composition), "あう");
        assert_eq!(composition.get_cursor_pos(), 1);
    }

    // ============================================================
    // クリア
    // ============================================================

    #[test]
    fn test_clear() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.clear();

        assert_eq!(composition_to_string(&composition), "");
        assert_eq!(composition.get_content_units_len(), 0);
        assert_eq!(composition.get_cursor_pos(), 0);
    }

    // ============================================================
    // カーソル右移動
    // ============================================================

    #[test]
    fn test_move_cursor_right() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_to_head();

        composition.move_cursor_right();
        assert_eq!(composition.get_cursor_pos(), 1);

        composition.move_cursor_right();
        assert_eq!(composition.get_cursor_pos(), 2);

        composition.move_cursor_right();
        assert_eq!(composition.get_cursor_pos(), 3);
    }

    #[test]
    fn test_move_cursor_right_at_tail_does_nothing() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');

        composition.move_cursor_to_tail();
        composition.move_cursor_right();

        assert_eq!(composition.get_cursor_pos(), 2);
    }

    // ============================================================
    // カーソル左移動
    // ============================================================

    #[test]
    fn test_move_cursor_left() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_to_tail();

        composition.move_cursor_left();
        assert_eq!(composition.get_cursor_pos(), 2);

        composition.move_cursor_left();
        assert_eq!(composition.get_cursor_pos(), 1);

        composition.move_cursor_left();
        assert_eq!(composition.get_cursor_pos(), 0);
    }

    #[test]
    fn test_move_cursor_left_at_head_does_nothing() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');

        composition.move_cursor_to_head();
        composition.move_cursor_left();

        assert_eq!(composition.get_cursor_pos(), 0);
    }

    // ============================================================
    // Home / End
    // ============================================================

    #[test]
    fn test_move_cursor_to_head() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_to_head();

        assert_eq!(composition.get_cursor_pos(), 0);
    }

    #[test]
    fn test_move_cursor_to_tail() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_to_head();
        composition.move_cursor_to_tail();

        assert_eq!(composition.get_cursor_pos(), 3);
    }

    // ============================================================
    // 挿入・削除・カーソル移動の組み合わせ
    // ============================================================

    #[test]
    fn test_insert_delete_and_move_cursor() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('う');

        // あ｜う
        composition.move_cursor_left();

        // あ｜う → あい｜う
        composition.insert('い');

        assert_eq!(composition_to_string(&composition), "あいう");
        assert_eq!(composition.get_cursor_pos(), 2);

        // あ｜う
        composition.move_cursor_left();

        // あ｜いう → あう
        composition.delete();

        assert_eq!(composition_to_string(&composition), "あう");
        assert_eq!(composition.get_cursor_pos(), 1);

        // あ｜う → ｜あう
        composition.move_cursor_to_head();

        // ｜あう → あう
        composition.delete();

        assert_eq!(composition_to_string(&composition), "う");
        assert_eq!(composition.get_cursor_pos(), 0);
    }

    #[test]
    fn test_backspace_and_delete_together() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        // あい｜う
        composition.move_cursor_left();

        // あ｜う
        composition.backspace();

        assert_eq!(composition_to_string(&composition), "あう");
        assert_eq!(composition.get_cursor_pos(), 1);

        // あ｜う → あ｜ 
        composition.delete();

        assert_eq!(composition_to_string(&composition), "あ");
        assert_eq!(composition.get_cursor_pos(), 1);
    }

    // ============================================================
    // 境界値
    // ============================================================

    #[test]
    fn test_empty_composition_operations() {
        let mut composition = Composition::new();

        composition.backspace();
        composition.delete();
        composition.move_cursor_left();
        composition.move_cursor_right();
        composition.move_cursor_to_head();
        composition.move_cursor_to_tail();

        assert_eq!(composition_to_string(&composition), "");
        assert_eq!(composition.get_content_units_len(), 0);
        assert_eq!(composition.get_cursor_pos(), 0);
    }

    #[test]
    fn test_cursor_never_exceeds_length() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        for _ in 0..10 {
            composition.move_cursor_right();
        }

        assert_eq!(composition.get_cursor_pos(), 3);
    }

    #[test]
    fn test_cursor_never_becomes_negative() {
        let mut composition = Composition::new();

        composition.insert('あ');
        composition.insert('い');
        composition.insert('う');

        composition.move_cursor_to_head();

        for _ in 0..10 {
            composition.move_cursor_left();
        }

        assert_eq!(composition.get_cursor_pos(), 0);
    }

    // ============================================================
    // Unicode
    // ============================================================

    #[test]
    fn test_unicode_characters() {
        let mut composition = Composition::new();

        composition.insert('漢');
        composition.insert('字');
        composition.insert('😀');

        assert_eq!(composition_to_string(&composition), "漢字😀");
        assert_eq!(composition.get_content_units_len(), 3);
        assert_eq!(composition.get_cursor_pos(), 3);

        composition.backspace();

        assert_eq!(composition_to_string(&composition), "漢字");
        assert_eq!(composition.get_cursor_pos(), 2);
    }
}