pub mod buffer;
pub mod editor;
pub mod hex;
pub mod model;
pub mod piece_table;
pub mod search;

pub use buffer::{DocumentBuffer, SUPPORTED_ENCODINGS};
#[allow(unused_imports)]
pub use editor::{
    char_col_to_visual_col, char_visual_columns, expand_tabs, visual_col_to_char_col, EditorState,
    Selection, SelectionKind, TextPosition, Viewport, VisualBounds, VisibleLineData,
};
#[allow(unused_imports)]
pub use model::{EditMode, NewLineFormat};
#[allow(unused_imports)]
pub use piece_table::{BufferSource, LineIndex, OriginalBuffer, Piece, PieceTable};
#[allow(unused_imports)]
pub use search::{
    build_regex, count_matches, find_all_matches, find_hex_matches, find_next_match, find_prev_match,
    parse_hex_query, replace_all_matches, SearchMatch, SearchOptions,
};
