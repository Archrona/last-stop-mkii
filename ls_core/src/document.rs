//! A buffer of text organized into lines. Equipped with undo, redo, and anchors.
//!
//! Supports advanced language features, parsing, and many other useful features
//! that enable speech coding.

use crate::oops::Oops;

//-----------------------------------------------------------------------------

/// A row-column position in a [`Document`].
/// 
/// Positions are indexed from 0. All unicode codepoints count for 1 character.
/// Emojis like 👋🏻 are two codepoints (0x1F44B, 0x1F3FB), and take up two 
/// logical columns. Tabs are one codepoint. For this reason,
/// the *visual* position of text on screen is not necessarily the same as
/// the row and column in the document.
///
/// Legal position columns are up to *and including* the length of the line.
/// This is because we can insert characters or position a cursor after the
/// last character of a line.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Debug, Default)]
pub struct Position {
    pub row: usize,
    pub column: usize
}

/// A location in a [`Document`] which keeps its position when the document is
/// changed.
/// 
/// The typical purpose of anchors is to represent cursors, breakpoints,
/// folding, tooltips, and other information that needs to be "attached"
/// to some location in the document.
///
/// # Standard Anchors
///
/// All documents contain two anchors: a cursor and a mark.
///
/// The cursor is the "primary" position that the user has selected.
/// If the user has chosen a range, e.g. by dragging a span with the mouse,
/// then the mark will track the starting point of the selection,
/// and the cursor will track the most recent point in the selection
/// (for example, following the mouse).
///
/// The cursor's index in the document is `Anchor::CURSOR`.
/// The mark's index in the document is `Anchor::MARK`.
///
/// # Performance
///
/// This implementation does not scale well to large numbers of anchors. 
/// Changes to documents incur a `O(n)` cost where `n` is the number of anchors.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Debug, Default)]
pub struct Anchor {
    pub position: Position
}

/// A region in a document with a beginning and ending [`Position`].
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Range {
    pub beginning: Position,
    pub ending: Position
}

/// An indentation policy (spaces or tabs-and-spaces) and a tab width.
///
/// # Limitations
/// At the moment, [`Indentation`] is not able to represent the variable
/// tab widths which sometimes occur in languages like Haskell where
/// it is customary to align multi-line elements based on the contents
/// of the lines rather than a fixed-width tab size. 
///
/// In short, it makes sense to limit [`Indentation`] to representations which
/// do not require semantic knowledge about particular languages.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct Indentation {
    pub use_spaces: bool,
    pub spaces_per_tab: usize
}


/// A reification of a reversible modification to a [`Document`].
///
/// When a change is **applied**, the document is modified and the inverse
/// change is returned. This is used to populate the undo and redo stacks
/// in [`Document`]. In short, if a client requests, for example, an insertion,
/// a matching removal is returned by `.apply()`.
///
/// As a design consideration, the inverse of a change is always 
/// *exactly* one change. For this reason, for instance, [`Change::Insert`]
/// does not modify anchors, since this would require [`Change::Remove`] to
/// store a list of [`Anchor`] changes, and this would duplicate the
/// functionality of [`Change::AnchorSet`]. When adding new change types,
/// prefer to use a larger number of changes which factor into small,
/// easily reversible modifications.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Change {

    /// Represents inserting `text` at `position` - literally, no escapes,
    /// exactly the characters that get inserted.
    Insert { text: Vec<Vec<char>>, position: Position },

    /// Represents removing the text within `range`.
    Remove { range: Range },

    /// Represents changing the contents of previously existing anchor
    /// at `index` to `value`.
    AnchorSet { index: usize, value: Anchor },

    /// Represents inserting a new anchor equal to `value`
    /// at `index`.
    AnchorInsert { index: usize, value: Anchor },

    /// Represents removing the anchor at `index`, shifting subsequent
    /// anchors to the left by one.
    AnchorRemove { index: usize },

    /// Represents a change to the indentation policy.
    IndentationChange { value: Indentation }
}

/// A series of [`Change`] to be applied as a group.
/// 
/// Because individual changes are typically rather small atoms, user actions
/// (e.g. pressing Ctrl-Z) undo entire [`ChangePacket`]s. 
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ChangePacket {
    changes: Vec<Change>
}



pub struct InsertOptions {
    escapes: bool,
    indent: bool,
    spacing: bool,
    range: Option<Range>
}



pub struct RemoveOptions {
    range: Option<Range>
}

pub type AnchorHandle = usize;

/// A buffer of text organized into lines. Equipped with undo, redo, and anchors.
/// The top-level struct for this module.
///
/// The [`Document`] is central to ls_core. Clients of ls_core are likely
/// to spend much of their time working with this type.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Document {
    lines: Vec<Vec<char>>,
    anchors: Vec<Anchor>,
    indentation: Indentation,
    undo_buffer: Vec<ChangePacket>,
    redo_buffer: Vec<ChangePacket>
}



//-----------------------------------------------------------------------------

impl Position {
    /// Returns the position `(row, column)`.
    #[inline(always)]
    pub fn from(row: usize, column: usize) -> Position {
        Position {
            row, column
        }
    }
}

impl Range {
    /// Returns the range from `(start_row, start_column)` to `(end_row, end_column)`.
    #[inline(always)]
    pub fn from(
        start_row: usize,
        start_column: usize,
        end_row: usize,
        end_column: usize
    ) -> Range {

        Range {
            beginning: Position::from(start_row, start_column),
            ending: Position::from(end_row, end_column)
        }
    }
}

impl ChangePacket {
    pub fn new() -> ChangePacket {
        ChangePacket {
            changes: vec![]
        }
    }
}

impl Indentation {
    /// Returns an all-spaces indentation poli6cy with each tab level `count`
    /// spaces apart.
    ///
    /// # Panics
    /// Panics if `count` is 0.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let indent = Indentation::spaces(3);
    /// assert_eq!(indent.produce(6), "      ");
    /// ```
    pub fn spaces(count: usize) -> Indentation {
        if count == 0 {
            panic!("Invalid indentation - must have non-zero spaces per indent");
        }

        Indentation {
            use_spaces: true,
            spaces_per_tab: count
        }
    }
    
    /// Returns a tabs-and-spaces indentation policy with each tab taking up
    /// `spaces_per_tab` spaces. If tabs and spaces are mixed, each tab is
    /// assumed to be equivalent to `spaces_per_tab` spaces, and margins
    /// produced by this `Indentation` start with as many tabs as possible and
    /// then wrap up the remainder with spaces.
    ///
    /// # Panics
    /// Panics if `spaces_per_tab` is 0.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let indent = Indentation::tabs(3);
    /// assert_eq!(indent.produce(6), "\t\t");
    /// assert_eq!(indent.produce(11), "\t\t\t  ");
    /// ```
    pub fn tabs(spaces_per_tab: usize) -> Indentation {
        if spaces_per_tab == 0 {
            panic!("Invalid indentation - must have non-zero spaces per tab");
        }

        Indentation {
            use_spaces: false,
            spaces_per_tab
        }
    }
    
    /// Returns `(spaces, bytes)` where `spaces` is the number of *logical spaces*
    /// in the left margin's whitespace (spaces count as 1, tabs count as `self.spaces_per_tab`),
    /// and `bytes` is the number of bytes that make up the left margin in `line`.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let indent = Indentation::spaces(2);
    /// assert_eq!(indent.measure("    "), (4, 4));
    /// assert_eq!(indent.measure("\t\t Hello \t there"), (5, 3));
    /// ```
    pub fn measure(&self, line: &str) -> (usize, usize) {
        let mut spaces: usize = 0;
        
        for (byte, c) in line.char_indices() {
            if c == ' ' {
                spaces += 1;
            } else if c == '\t' {
                spaces += self.spaces_per_tab;
            } else {
                return (spaces, byte);
            }
        }
        
        (spaces, line.len())
    }

    /// Returns the white space for a left margin with visual width of `spaces` spaces
    /// using either spaces or tabs-and-spaces.
    ///
    /// If this `Indentation` uses tabs and the requested number of spaces is not a
    /// multiple of `spaces_per_tab`, spaces will be used to complete the left margin.
    pub fn produce(&self, spaces: usize) -> String {
        if self.use_spaces {
            " ".repeat(spaces)
        } else {
            "\t".repeat(spaces / self.spaces_per_tab) + &" ".repeat(spaces % self.spaces_per_tab)
        }
    }

    /// Returns `line` indented by `indent_delta` tab stops.
    /// 
    /// If `indent_delta` is negative, this performs a dedent.
    /// If the dedent would reach past the left margin, `indent` returns an empty (zero-space)
    /// margin.
    ///
    /// If `include_content` is false, only return the left margin of `line` - omit the content
    /// that comes after it.
    ///
    /// ```
    /// use ls_core::document::*;
    /// assert_eq!(Indentation::spaces(4).indent("    Hello", -1, true), "Hello");
    /// assert_eq!(Indentation::spaces(4).indent("    Hello", -1, false), "");
    /// assert_eq!(Indentation::spaces(4).indent("    Hello", 1, true), "        Hello");
    /// assert_eq!(Indentation::spaces(4).indent("    Hello", 1, false), "        ");
    /// assert_eq!(Indentation::tabs(4).indent("     Hello", -1, true), " Hello");
    /// assert_eq!(Indentation::tabs(4).indent("     Hello", -1, false), " ");
    /// assert_eq!(Indentation::tabs(4).indent("     Hello", 1, true), "\t\t Hello");
    /// assert_eq!(Indentation::tabs(4).indent("     Hello", 1, false), "\t\t ");
    /// ```
    pub fn indent(&self, line: &str, indent_delta: isize, include_content: bool) -> String {
        let (spaces, bytes) = self.measure(line);
        let requested_spaces: isize = (spaces as isize) + indent_delta * (self.spaces_per_tab as isize);
        let actual_spaces: usize = if requested_spaces < 0 { 0 } else { requested_spaces as usize };
        
        let mut result = self.produce(actual_spaces);
        if include_content {
            result += &line[bytes..];
        }
        
        result
    }
}

impl Anchor {
    /// The index of the cursor in a document's anchor list.
    pub const CURSOR: usize = 0;

    /// The index of the mark in a document's anchor list.
    pub const MARK: usize = 1;
}

impl Change {
    /// Performs a `Change` on `document`, returning the inverse change.
    ///
    /// # Panics
    /// Panics if the change is impossible to apply or if any invariants
    /// of the document (positions are valid, and so on) are violated.
    /// 
    /// This module is responsible for ensuring that changes will not
    /// violate these invariants. If they do, it is a bug in our code,
    /// not the client code.
    fn apply_untracked(&self, document: &mut Document) -> Change {
        use Change::*;
    
        match self {
            Insert { text, position } =>        document.insert_untracked(&text, position),
            Remove { range } =>                 document.remove_untracked(range),
            AnchorSet { index, value } =>       document.set_anchor_untracked(*index, value),
            AnchorInsert { index, value } =>    document.insert_anchor_untracked(*index, value),
            AnchorRemove { index } =>           document.remove_anchor_untracked(*index),
            IndentationChange { value } =>      document.set_indentation_untracked(value)
        }
    }
    
}

impl Document {
    /// Returns an empty document with one empty line. This sets aside cursor and mark
    /// in the first two anchor indices (cursor at `Anchor::CURSOR`, mark at `Anchor::MARK`)
    /// and initializes them both to (0, 0).
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::new();
    /// assert_eq!(document.text(), "");
    /// assert_eq!(document.anchors().len(), 2);
    /// assert_eq!(
    ///     document.anchor(Anchor::CURSOR).unwrap().position,
    ///     Position { row: 0, column: 0 }
    /// );
    /// assert_eq!(document.undo_redo_depth(), (0, 0));
    /// ```
    pub fn new() -> Document {
        Document {
            lines: vec![vec![]],
            anchors: vec![Anchor::default(), Anchor::default()],
            indentation: Indentation::spaces(4),
            undo_buffer: vec![],
            redo_buffer: vec![]
        }
    }

    /// Returns a document initialized from `text`. This sets aside cursor and mark
    /// in the first two anchor indices (cursor at `Anchor::CURSOR`, mark at `Anchor::MARK`)
    /// and initializes them both to (0, 0).
    ///
    /// The resulting document is guaranteed to have at least one line, even if it is
    /// just the empty line. Trailing newlines are stripped ainnd the final empty line
    /// is not included.
    ///
    /// # Examples
    ///
    /// ```
    /// use ls_core::document::*;
    /// let empty = Document::from("");
    /// assert_eq!(empty, Document::new());
    /// ```
    ///
    /// ```
    /// use ls_core::document::*;
    /// let empty = Document::from("Hello\n  there!\n");
    /// assert_eq!(*empty.lines(), vec![
    ///     "Hello".chars().collect::<Vec<char>>(),
    ///     "  there!".chars().collect::<Vec<char>>()
    /// ]);
    /// ```
    pub fn from(text: &str) -> Document {
        let lines: Vec<Vec<char>> = if text == "" {
            vec![vec![]]
        } else {
            text.lines().map(|x| x.chars().collect::<Vec<char>>()).collect()
        };

        Document { 
            lines,
            anchors: vec![Anchor::default(), Anchor::default()],
            indentation: Indentation::spaces(4),
            undo_buffer: vec![],
            redo_buffer: vec![]
        }
    }

    /// Returns whether `position` is legal in this document. If a line contains 5
    /// characters, for instance, columns 0 through 5, inclusive, are legal.
    /// 
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\n  there!\n");
    /// assert_eq!(true, document.position_valid(&Position { row: 0, column: 0 }));
    /// assert_eq!(true, document.position_valid(&Position { row: 0, column: 5 }));
    /// assert_eq!(false, document.position_valid(&Position { row: 0, column: 6 }));
    /// assert_eq!(false, document.position_valid(&Position { row: 2, column: 0 }));
    /// ```
    pub fn position_valid(&self, position: &Position) -> bool {
        position.row < self.lines.len() && position.column <= self.lines[position.row].len()
    }

    /// Returns whether `range` is legal in this document. Both its beginning and new and
    /// ending positions must be in range, and its beginning cannot come after its ending.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\n  there!\n");
    ///
    /// let p_1 = Position { row: 0, column: 0 };
    /// let p_2 = Position { row: 0, column: 5 };
    /// let p_3 = Position { row: 0, column: 6 };
    /// let p_4 = Position { row: 1, column: 2 };
    /// let p_5 = Position { row: 2, column: 0 };
    /// 
    /// assert_eq!(true, document.range_valid(&Range { beginning: p_1, ending: p_1 }));
    /// assert_eq!(true, document.range_valid(&Range { beginning: p_1, ending: p_4 }));
    /// assert_eq!(true, document.range_valid(&Range { beginning: p_2, ending: p_4 }));
    /// assert_eq!(false, document.range_valid(&Range { beginning: p_2, ending: p_1 }));
    /// assert_eq!(false, document.range_valid(&Range { beginning: p_2, ending: p_3 }));
    /// assert_eq!(false, document.range_valid(&Range { beginning: p_5, ending: p_5 }));
    /// ```
    pub fn range_valid(&self, range: &Range) -> bool {
        self.position_valid(&range.beginning) 
            && self.position_valid(&range.ending) 
            && range.beginning <= range.ending
    }

    /// Returns the `index`th line as a `&String`, or `None` if out of bounds.
    pub fn line(&self, index: usize) -> Option<String> {
        if index >= self.lines.len() {
            None
        } else {
            Some(self.lines[index].iter().copied().collect())
        }
    }

    /// Returns the text of the document as a list of lines. This is guaranteed to contain
    /// at least one line.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\nthere");
    /// assert_eq!(*document.lines(), vec![
    ///     "Hello".chars().collect::<Vec<char>>(),
    ///     "there".chars().collect::<Vec<char>>()
    /// ]);
    /// ```
    pub fn lines(&self) -> &Vec<Vec<char>> {
        &self.lines
    }


    /// Returns the number of rows in the document. Will always be at least 1.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// assert_eq!(Document::new().rows(), 1);
    /// let document = Document::from("Hello\nthere\ncaptain!");
    /// assert_eq!(document.rows(), 3);
    /// ```
    pub fn rows(&self) -> usize {
        self.lines.len()
    }

    /// Returns a list of anchors. This list is guaranteed to contain the cursor at index
    /// 0 and the mark at index 1.
    pub fn anchors(&self) -> &Vec<Anchor> {
        &self.anchors
    }

    /// Returns the anchor at index `index`, or `None` if out of bounds.
    pub fn anchor(&self, index: usize) -> Option<&Anchor> {
        self.anchors.get(index)
    }

    /// Returns the cursor.
    pub fn cursor(&self) -> Anchor {
        self.anchors[0]
    }

    /// Returns the mark.
    pub fn mark(&self) -> Anchor {
        self.anchors[1]
    }

    /// Returns `(u, r)`, where `u` is the number of undo operations we can perform,
    /// and `r` is the number of redo operations we can perform.
    pub fn undo_redo_depth(&self) -> (usize, usize) {
        (self.undo_buffer.len(), self.redo_buffer.len())
    }

    /// Returns the document as a single string with lines separated by "\n".
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\nthere\ncaptain!");
    /// assert_eq!(document.text(), "Hello\nthere\ncaptain!".to_string());
    /// ```
    pub fn text(&self) -> String {
        self.lines.iter().map(|x| x.iter().collect::<String>())
            .collect::<Vec<String>>().join("\n")
    }

    /// Returns the range as a single string with lines separated by "\n",
    /// or None if the range is invalid.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\nthere\ncaptain!");
    /// assert_eq!(document.text_range(&Range::from(0, 0, 0, 0)), Some("".to_string()));
    /// assert_eq!(document.text_range(&Range::from(0, 0, 0, 1)), Some("H".to_string()));
    /// assert_eq!(document.text_range(&Range::from(0, 2, 0, 5)), Some("llo".to_string()));
    /// assert_eq!(document.text_range(&Range::from(0, 0, 1, 0)), Some("Hello\n".to_string()));
    /// assert_eq!(document.text_range(&Range::from(0, 2, 2, 3)), Some("llo\nthere\ncap".to_string()));
    /// assert_eq!(document.text_range(&Range::from(0, 5, 1, 0)), Some("\n".to_string()));
    /// assert_eq!(document.text_range(&Range::from(0, 0, 0, 10)), None);
    /// assert_eq!(document.text_range(&Range::from(1, 1, 0, 2)), None);    
    /// ```
    pub fn text_range(&self, range: &Range) -> Option<String> {
        if !self.range_valid(range) {
            None
        } else {
            let mut s = String::new();

            if range.beginning.row == range.ending.row {
                s.extend(
                    self.lines[range.beginning.row][range.beginning.column..range.ending.column]
                    .iter()
                );
            } else {
                s.extend(self.lines[range.beginning.row][range.beginning.column..].iter());

                for line in self.lines[(range.beginning.row + 1)..range.ending.row].iter() {
                    s += "\n";
                    s.extend(line.iter());
                }

                s += "\n";
                s.extend(self.lines[range.ending.row][..range.ending.column].iter());
            }

            Some(s)
        }
    }



    pub fn insert(&mut self, text: &String, options: &InsertOptions) -> Result<(), Oops> {
        todo!();
    }
        
    pub fn remove(&mut self, options: &RemoveOptions) -> Result<(), Oops> {
        todo!();
    }
    
    pub fn set_anchor_position(&mut self, index: usize, position: &Position) -> Result<(), Oops> {
        todo!();
    }
    
    pub fn create_anchor(&mut self, position: &Position) -> Result<AnchorHandle, Oops> {
        todo!();
    }
    
    







    
    /// Inserts `text`, a list of one or more lines, into the document at `position`.
    /// Returns the `Change` which would undo this modification.
    /// 
    /// This does not process escapes, indentation, spacing, or capitalization.
    /// The *only* thing it does is insert exactly what it is told to.
    ///
    /// # Panics
    /// Panics if asked to insert 0 lines or if `position` is out of range.
    fn insert_untracked(&mut self, text: &Vec<Vec<char>>, position: &Position) -> Change {
        if text.len() == 0 {
            panic!("cannot insert 0 lines");
        }
        self.assert_position_valid(position);

        let after = self.lines[position.row].drain(position.column..).collect::<Vec<char>>();

        if text.len() == 1 {
            self.lines[position.row].extend(text[0].iter());
            self.lines[position.row].extend(after.iter());
        } else {
            self.lines[position.row].extend(text[0].iter());
            let to_append = text.iter().skip(1).cloned().collect::<Vec<Vec<char>>>();
            
            push_all_at(&mut self.lines, position.row + 1, &to_append);
            self.lines[position.row + text.len() - 1].extend(after.iter());
        }

        Change::Remove { range: Range {
            beginning: *position,
            ending: Position { 
                row: position.row + text.len() - 1,
                column: text[text.len() - 1].len()
            }
        }}
    }
    
    /// Removes the text at `range`.
    /// Returns the `Change` which would undo this modification.
    ///
    /// This does not process escapes, indentation, spacing, or capitalization.
    ///
    /// # Panics
    /// Panics if `range` is invalid (out of bounds, reversed).
    fn remove_untracked(&mut self, range: &Range) -> Change {
        self.assert_range_valid(range);

        if range.beginning.row == range.ending.row {
            Change::Insert {
                text: vec![self.lines[range.beginning.row]
                    .drain(range.beginning.column..range.ending.column)
                    .collect::<Vec<char>>()],
                position: range.beginning
            }
        } else {
            let mut lines = Vec::new();

            lines.push(
                self.lines[range.beginning.row]
                    .drain(range.beginning.column..)
                    .collect::<Vec<char>>()
            );

            let trailing = self.lines[range.ending.row]
                .drain(range.ending.column..)
                .collect::<Vec<char>>();

            self.lines[range.beginning.row].extend(trailing);

            lines.extend(
                self.lines
                    .drain((range.beginning.row + 1)..= range.ending.row)
                    .map(|x| x.iter().copied().collect::<Vec<char>>())
            );

            Change::Insert {
                text: lines,
                position: range.beginning
            }
        }
    }
    
    /// Sets the content of anchor at index `index` to `value`.
    /// Returns the `Change` which would undo this modification.
    ///
    /// # Panics
    /// Panics if index is out of range or if the anchor points to an invalid position.
    fn set_anchor_untracked(&mut self, index: usize, value: &Anchor) -> Change {
        if index >= self.anchors.len() {
            panic!("set_anchor_untracked: invalid index {}", index);
        }
        self.assert_position_valid(&value.position);
        
        let orig_value = self.anchors[index];
        self.anchors[index] = *value;
        
        Change::AnchorSet { index, value: orig_value }
    }
    
    /// Inserts a new anchor at `index` with value `value`.
    /// Returns the `Change` which would undo this modification.
    ///
    /// Indices 0 and 1 are used for the cursor and mark, so no
    /// new anchors can be inserted before index 2.
    ///
    /// # Panics
    /// Panics if `index` is out of range or if the anchor points to an invalid position.
    fn insert_anchor_untracked(&mut self, index: usize, value: &Anchor) -> Change {
        if index < 2 || index > self.anchors.len() {
            panic!("insert_anchor_untracked: invalid index {}", index);
        }
        self.assert_position_valid(&value.position);
        
        self.anchors.insert(index, *value);

        Change::AnchorRemove { index }
    }
    
    /// Removes the anchor at index `index`.
    /// Returns the `Change` which would undo this modification.
    ///
    /// Indices 0 and 1 are used for the cursor and mark, so no
    /// new anchors can be inserted before index 2.
    ///
    /// # Panics
    /// Panics if `index` is out of range.
    fn remove_anchor_untracked(&mut self, index: usize) -> Change {
        if index < 2 || index >= self.anchors.len() {
            panic!("remove_anchor_untracked: invalid index {}", index);
        }
        
        let orig_value = self.anchors[index];
        self.anchors.remove(index);
        
        Change::AnchorInsert { index, value: orig_value }     
    }

    /// Sets the indentation policy.
    fn set_indentation_untracked(&mut self, value: &Indentation) -> Change {
        let reverse = Change::IndentationChange { value: self.indentation };
        self.indentation = *value;
        
        reverse
    }

    /// Asserts that a position is valid.
    ///
    /// # Panics
    /// Panics if `position` is out of bounds.
    fn assert_position_valid(&self, position: &Position) -> () {
        assert!(self.position_valid(position));
    }

    /// Asserts that a range is valid (start and end positions are both valid,
    /// start does not come after end.)
    ///
    /// # Panics
    /// Panics if `range` is invalid.
    fn assert_range_valid(&self, range: &Range) -> () {
        assert!(self.range_valid(range));
    }
}

/// Pushes all items from `s` into `v` starting at index `offset`.
///
/// `v` must contain items with trait Clone and Default. This uses
/// a *somewhat* efficient O(n) method via `Vec::swap`.
///
/// Author: swizard <https://stackoverflow.com/a/28687253>
///
/// # Examples
/// ```
/// use ls_core::document::*;
/// let mut items = vec![3, 7, 1];
/// push_all_at(&mut items, 0, &[0, 2]);
/// assert_eq!(items, &[0, 2, 3, 7, 1]);
/// push_all_at(&mut items, 0, &[]);
/// assert_eq!(items, &[0, 2, 3, 7, 1]);
/// push_all_at(&mut items, 3, &[10, 11]);
/// assert_eq!(items, &[0, 2, 3, 10, 11, 7, 1]);
/// push_all_at(&mut items, 7, &[12, 13]);
/// assert_eq!(items, &[0, 2, 3, 10, 11, 7, 1, 12, 13]);
/// ```
pub fn push_all_at<T>(v: &mut Vec<T>, mut offset: usize, s: &[T]) where T: Clone + Default {
    match (v.len(), s.len()) {
        (_, 0) => (),
        (0, _) => { v.append(&mut s.to_owned()); },
        (_, _) => {
            assert!(offset <= v.len());
            let pad = s.len() - ((v.len() - offset) % s.len());
            v.extend(std::iter::repeat(Default::default()).take(pad));
            v.append(&mut s.to_owned());
            let total = v.len();
            while total - offset >= s.len() {
                for i in 0 .. s.len() { v.swap(offset + i, total - s.len() + i); }
                offset += s.len();
            }
            v.truncate(total - pad);
        },
    }
}



//-----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_anchor_untracked() {
        let mut document = Document::from("AAA\nBBB");
        let inverse = document.set_anchor_untracked(Anchor::CURSOR, &Anchor {
            position: Position { row: 1, column: 3 }
        });

        assert_eq!(document.cursor().position, Position { row: 1, column: 3 });

        assert_eq!(inverse, Change::AnchorSet {
            index: Anchor::CURSOR,
            value: Anchor {
                position: Position { row: 0, column: 0 }
            }
        });
    }

    #[test]
    fn insert_remove_anchor_untracked() {
        let mut document = Document::from("AAA\nBBB");
        let inverse = document.insert_anchor_untracked(document.anchors().len(), &Anchor {
            position: Position { row: 1, column: 3 }
        });

        assert_eq!(document.anchor(2).unwrap().position, Position { row: 1, column: 3 });
        assert_eq!(inverse, Change::AnchorRemove { index: 2 });

        let inverse_2 = inverse.apply_untracked(&mut document);

        assert_eq!(document.anchors().len(), 2);
        assert_eq!(inverse_2, Change::AnchorInsert {
            index: 2,
            value: Anchor {
                position: Position { row: 1, column: 3 }
            }
        });
    }

    #[test]
    fn insert_untracked() {
        let mut document = Document::from("AAA\nBBB");
        
        assert_eq!(document.insert_untracked(
            &vec!["hello".chars().collect()],
            &Position { row: 0, column: 0 }
        ), Change::Remove { range: Range {
            beginning: Position { row: 0, column: 0 },
            ending: Position { row: 0, column: 5 }
        }});
        assert_eq!(document.text(), "helloAAA\nBBB");
        
        assert_eq!(document.insert_untracked(
            &vec!["there".chars().collect(), "friend".chars().collect()],
            &Position { row: 1, column: 2 }
        ), Change::Remove { range: Range {
            beginning: Position { row: 1, column: 2 },
            ending: Position { row: 2, column: 6 }
        }});
        assert_eq!(document.text(), "helloAAA\nBBthere\nfriendB");

        document.insert_untracked(
            &vec!["ly".chars().collect()],
            &Position { row: 2, column: 7 }
        );
        assert_eq!(document.text(), "helloAAA\nBBthere\nfriendBly");
    }

    #[test]
    fn unicode() {
        let mut document = Document::from("🙈我爱unicode🦄\n매우 짜증나");
        assert_eq!(document.lines()[0][0], '🙈');
        assert_eq!(document.lines()[0][1], '我');
        assert_eq!(document.lines()[0][10], '🦄');
        assert_eq!(document.lines()[1][1], '우');
        assert_eq!(document.text(), "🙈我爱unicode🦄\n매우 짜증나");

        let chg = document.insert_untracked(&vec![
            "👋🏻🤚🏻🖐🏻✋🏻🖖🏻👌🏻".chars().collect(),
            "⌚️📱📲💻⌨️".chars().collect(),
            "".chars().collect()
        ], &Position::from(1, 0));
        assert_eq!(document.text(), "🙈我爱unicode🦄\n👋🏻🤚🏻🖐🏻✋🏻🖖🏻👌🏻\n⌚️📱📲💻⌨️\n매우 짜증나");

        // Some emojis are two codepoints in a row...
        // We don't handle that. Nope.
        // (1, 6) is just after 👋🏻🤚🏻🖐🏻
        // (2, 3) is just after ⌚️📱
        let chg_2 = document.remove_untracked(&Range::from(1, 6, 2, 3));
        assert_eq!(document.text(), "🙈我爱unicode🦄\n👋🏻🤚🏻🖐🏻📲💻⌨️\n매우 짜증나");

        chg_2.apply_untracked(&mut document);
        assert_eq!(document.text(), "🙈我爱unicode🦄\n👋🏻🤚🏻🖐🏻✋🏻🖖🏻👌🏻\n⌚️📱📲💻⌨️\n매우 짜증나");

        chg.apply_untracked(&mut document);
        assert_eq!(document.text(), "🙈我爱unicode🦄\n매우 짜증나");
    }

    #[test]
    fn remove_untracked() {
        let mut document = Document::from("01234\nabcde\nABCDE");

        assert_eq!(
            document.remove_untracked(&Range::from(1, 2, 1, 2)),
            Change::Insert {
                text: vec!["".chars().collect()],
                position: Position::from(1, 2)
            }
        );
        assert_eq!(document.text(), "01234\nabcde\nABCDE");

        assert_eq!(
            document.remove_untracked(&Range::from(1, 2, 1, 4)),
            Change::Insert {
                text: vec!["cd".chars().collect()],
                position: Position::from(1, 2)
            }
        );
        assert_eq!(document.text(), "01234\nabe\nABCDE");

        assert_eq!(
            document.remove_untracked(&Range::from(0, 4, 1, 1)),
            Change::Insert {
                text: vec!["4".chars().collect(), "a".chars().collect()],
                position: Position::from(0, 4)
            }
        );
        assert_eq!(document.text(), "0123be\nABCDE");
    }
}