//! A buffer of text organized into lines. Equipped with undo, redo, and anchors.
//!
//! Supports advanced language features, parsing, and many other useful features
//! that enable speech coding.

use crate::util::Oops;
use std::collections::hash_map;
use tree_sitter;
use crate::language;
use crate::util;
use crate::util::{substring, slice};
use std::fmt;

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
/// The cursor's handle is `Anchor::CURSOR`.
/// The mark's handle is `Anchor::MARK`.
///
/// # Performance
///
/// This implementation does not scale well to large numbers of anchors. 
/// Insertions and deletions incur a `O(n)` cost where `n` is the number of anchors.
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
    Insert { text: Vec<String>, position: Position },

    /// Represents removing the text within `range`.
    Remove { range: Range },

    /// Represents changing the contents of previously existing anchor
    /// at `handle` to `value`.
    AnchorSet { handle: AnchorHandle, value: Anchor },

    /// Represents inserting a new anchor equal to `value`
    /// at `handle`.
    AnchorInsert { handle: AnchorHandle, value: Anchor },

    /// Represents removing the anchor at `handle`, shifting subsequent
    /// anchors to the left by one.
    AnchorRemove { handle: AnchorHandle },

    /// Represents a change to the indentation policy.
    IndentationChange { value: Indentation },

    /// Represents a change in the document's language string.
    LanguageChange { value: String },

}

/// A series of [`Change`] to be applied as a group.
/// 
/// Because individual changes are typically rather small atoms, user actions
/// (e.g. pressing Ctrl-Z) undo entire [`ChangePacket`]s. 
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ChangePacket {
    changes: Vec<Change>
}


/// Options for [`Document::insert`].
///
/// Inserting elements into a document is a complicated operation.
/// This allows callers to easily specify multiple insert operations using
/// sensible defaults like [`InsertOptions::exact`].
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct InsertOptions {
    /// Should the insert operation escape commands like $u (indent), $d (dedent),
    /// $n (newline), $g (glue), and so forth?
    /// 
    /// These escapes are used by speech editing to perform special operations.
    pub escapes: bool,

    /// Should the insert automatically indent Lines after the first?
    pub indent: bool,

    /// Should the insert attempt to either insert or remove whitespace
    /// immediately before and immediately after the inserted content
    /// in a language-specific manner?
    pub spacing: bool,

    /// If `None`, the insert takes place between the cursor and mark.
    /// Otherwise, the insert takes place at this range.
    pub range: Option<Range>
}


/// Options for [`Document::remove`].
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct RemoveOptions {
    /// If `None`, the removal takes place between the cursor and mark.
    /// Otherwise, this range is removed.
    pub range: Option<Range>
}

/// An opaque-ish handle which acts as a unique key within a document for
/// anchors. The cursor is locked to [`Anchors::CURSOR`] and the mark is
/// locked to [`Anchors::MARK`], but no assumptions should be made as to the
/// handles assigned to other anchors.
pub type AnchorHandle = u32;


/// A container for [`Anchor`]s on a per-document basis.
/// 
/// Responsible for assigning unique handles ([`AnchorHandle`]) to each
/// anchor. 
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Anchors {
    store: hash_map::HashMap<u32, Anchor>,
    next_id: AnchorHandle
}

/// Represents a contextual region within a document.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ChainRegion {
    pub kind: String,
    pub range: Range
}

/// Represents a series of nested contextual regions within a document.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Chain {
    pub regions: Vec<ChainRegion>
}

/// Maintains the undo and redo stacks for a [`Document`].
/// 
/// A single editing command (insert, remove, etc.) can result in many
/// reversible changes which must be tracked in order to undo the command.
/// For this reason, we track changes in groups called [`ChangePacket`]s.
/// If an undo or redo command is issued, it is performed at the packet
/// level of granularity.
/// 
/// To indicate that a new packet should begin with the next [`Change`]
/// tracked, use [`UndoRedoStacks::checkpoint`].
/// 
/// Change tracking takes a quantity of memory not too much greater than
/// the total UTF-8 payload of all insertions and removals. However, for
/// long-running editing processes or for very large files, this change
/// tracking can become a memory burden. To signal that the undo and redo
/// stacks should be cleared, freeing this memory, use 
/// [`UndoRedoStacks::forget_everything`].
#[derive(Clone, Debug)]
pub struct UndoRedoStacks {
    undo_stack: Vec<ChangePacket>,
    redo_stack: Vec<ChangePacket>,
    checkpoint_requested: bool
}

/// A line of text stored in a document. Maintains its own length so that
/// we do not have to make O(n) queries to `.chars().count()`.
#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub struct Line {
    pub content: String,
    pub length: usize
}

/// A buffer of text organized into lines. Equipped with undo, redo, and anchors.
/// The top-level struct for this module.
///
/// The [`Document`] is central to ls_core. Clients of ls_core are likely
/// to spend much of their time working with this type.
pub struct Document {
    lines: Vec<Line>,
    anchors: Anchors,
    indentation: Indentation,
    undo_redo: UndoRedoStacks,

    language: String,
    parser: Option<tree_sitter::Parser>,
    tree: Option<tree_sitter::Tree>
}



//-----------------------------------------------------------------------------

impl Line {
    /// Returns the line containing `content`.
    pub fn from(content: String) -> Line {
        let length = content.chars().count();
        Line { content, length }
    }
}

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

    /// Returns true if the range starts and ends at the same position.
    pub fn empty(&self) -> bool {
        self.beginning == self.ending
    }
}



impl Indentation {
    /// Returns an all-spaces indentation policy with each tab level `count`
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
            let mut result = "\t".repeat(spaces / self.spaces_per_tab);
            result.push_str(&" ".repeat(spaces % self.spaces_per_tab));
            result
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
    /// assert_eq!(Indentation::spaces(4).indent(&"    Hello", -1, true), "Hello");
    /// assert_eq!(Indentation::spaces(4).indent(&"    Hello", -1, false), "");
    /// assert_eq!(Indentation::spaces(4).indent(&"    Hello", 1, true), "        Hello");
    /// assert_eq!(Indentation::spaces(4).indent(&"    Hello", 1, false), "        ");
    /// assert_eq!(Indentation::tabs(4).indent(&"     Hello", -1, true), " Hello");
    /// assert_eq!(Indentation::tabs(4).indent(&"     Hello", -1, false), " ");
    /// assert_eq!(Indentation::tabs(4).indent(&"     Hello", 1, true), "\t\t Hello");
    /// assert_eq!(Indentation::tabs(4).indent(&"     Hello", 1, false), "\t\t ");
    /// ```
    pub fn indent(&self, line: &str, indent_delta: isize, include_content: bool) -> String {
        let (spaces, byte_cutoff) = self.measure(line);
        let requested_spaces: isize = (spaces as isize) + indent_delta * (self.spaces_per_tab as isize);
        let actual_spaces: usize = if requested_spaces < 0 { 0 } else { requested_spaces as usize };
        
        let mut result = self.produce(actual_spaces);
        if include_content {
            result.push_str(&line[byte_cutoff..]);
        }
        
        result
    }
}

impl InsertOptions {
    /// Returns insert options which indicate the inserted text should be placed into
    /// the document with no escapes, indentation, or spacing at the current selection.
    pub fn exact() -> InsertOptions {
        InsertOptions {
            escapes: false,
            indent: false,
            spacing: false,
            range: None
        }
    }
    
    /// Returns insert options which indicate the inserted text should be placed into
    /// the document with no escapes, indentation, or spacing at `range`.
    pub fn exact_at(range: &Range) -> InsertOptions {
        InsertOptions {
            range: Some(*range),
            ..Self::exact()
        }
    }
}

impl RemoveOptions {
    /// Returns remove options which indicate a normal removal of the current selection
    /// with no special options.
    pub fn exact() -> RemoveOptions {
        RemoveOptions {
            range: None
        }
    }

    /// Returns remove options which indicate a normal removal at `range` with no
    /// special options.
    pub fn exact_at(range: &Range) -> RemoveOptions {
        RemoveOptions {
            range: Some(*range),
            ..Self::exact()
        }
    }
}

impl Anchor {
    /// Creates an anchor at position (0, 0).
    pub fn new() -> Anchor {
        Anchor {
            position: Default::default()
        }
    }

    /// Creates an anchor at position (`row`, `column`).
    pub fn from(row: usize, column: usize) -> Anchor {
        Anchor {
            position: Position::from(row, column),
            ..Default::default()
        }
    }
}

impl Anchors {
    /// The id of the cursor in a document's anchor list.
    pub const CURSOR: AnchorHandle = 0;

    /// The id of the mark in a document's anchor list.
    pub const MARK: AnchorHandle = 1;

    /// Returns a new [`Anchors`] with just a cursor and mark at position
    /// (0, 0).
    fn new() -> Anchors {
        let mut store = hash_map::HashMap::new();
        store.insert(Anchors::CURSOR, Anchor::new());
        store.insert(Anchors::MARK, Anchor::new());
        
        Anchors {
            store,
            next_id: 2 as AnchorHandle
        }
    }
    
    /// Returns the cursor (the primary anchor of a document). This
    /// [`Anchor`] is guaranteed to exist.
    fn cursor(&self) -> &Anchor {
        self.store.get(&Anchors::CURSOR).unwrap()
    }
    
    /// Returns the mark (the secondary anchor of a document). This
    /// [`Anchor`] is guaranteed to exist.
    fn mark(&self) -> & Anchor {
        self.store.get(&Anchors::MARK).unwrap()
    }
    
    /// Returns the anchor with handle `handle`, or `None` if the handle
    /// is not valid.
    fn get(&self, handle: AnchorHandle) -> Option<&Anchor> {
        self.store.get(&handle)
    }
    
    /// Sets the anchor with handle `handle` to `value`. Fails if `handle` does not
    /// exist.
    fn set(&mut self, handle: AnchorHandle, value: &Anchor) -> Result<Anchor, Oops> {
        match self.store.get_mut(&handle) {
            None => Err(Oops::NonexistentAnchor(handle)),
            Some(anchor) => {
                let old = anchor.clone();
                *anchor = *value;
                Ok(old)
            }
        }
    }
    
    /// Creates a new anchor with contents `anchor`. 
    /// 
    /// If `force_handle` is not `None`, the new anchor will
    /// use handle `force_handle`. This feature is not meant to be used
    /// directly by client code, but by undo-redo functionality which needs
    /// to roll the state back deterministically.
    fn create(&mut self, anchor: Anchor, force_handle: Option<AnchorHandle>) -> AnchorHandle {
        let handle = match force_handle {
            None => self.get_new_handle(),
            Some(h) => h
        };              
        
        self.store.insert(handle, anchor);
        handle
    }
    
    /// Removes the anchor with handle `handle`. Fails if `handle` does not exist.
    fn remove(&mut self, handle: AnchorHandle) -> Result<Anchor, Oops> {
        if handle == Anchors::CURSOR || handle == Anchors::MARK {
            Err(Oops::CannotRemoveAnchor(handle))
        } else {
            match self.store.remove(&handle) {
                None => Err(Oops::NonexistentAnchor(handle)),
                Some(old) => Ok(old)
            }
        }
    }

    /// Returns an iterator over all (handle, anchor) pairs, in no
    /// particular order.
    fn iter(&self) -> hash_map::Iter<'_, AnchorHandle, Anchor> {
        self.store.iter()
    }

    /// Generates a new, unused [`AnchorHandle`], incrementing the internal
    /// counter so that it remains unique.
    fn get_new_handle(&mut self) -> AnchorHandle {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl ChainRegion {
    /// Returns the `ChainRegion` with the given `kind` and `range`.
    pub fn from(kind: &str, range: &Range) -> ChainRegion {
        ChainRegion {
            kind: String::from(kind),
            range: range.clone()
        }
    }
}

impl fmt::Display for ChainRegion {
    /// Formats a `ChainRegion` for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {})-({}, {})",
            self.kind,
            self.range.beginning.row,
            self.range.beginning.column,
            self.range.ending.row,
            self.range.ending.column
        )
    }
}

impl Chain {
    /// Returns a new, empty `Chain`.
    pub fn new() -> Chain {
        Chain {
            regions: vec![]
        }
    }
    
    /// Pushes a new region onto a `Chain`. Corrects tree sitter's byte ranges
    /// into ls_core's Unicode codepoint indices.
    /// 
    /// # Panics
    /// Will panic if the byte indices are invalid.
    pub fn push(&mut self, kind: &str, range: tree_sitter::Range, doc: &Document) -> () {
        self.regions.push(ChainRegion::from(
            kind,
            &Range::from(
                range.start_point.row,
                util::byte_index_to_cp(
                    &doc.line(range.start_point.row).unwrap(),
                    range.start_point.column
                ).unwrap(),

                range.end_point.row,
                util::byte_index_to_cp(
                    &doc.line(range.end_point.row).unwrap(),
                    range.end_point.column
                ).unwrap()
            )
        ));
    }
}

impl fmt::Display for Chain {
    /// Formats a `Chain` for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { 
        for c in &self.regions {
            write!(f, "{}\n", &c)?;
        }

        fmt::Result::Ok(())
    }
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
            AnchorSet { handle, value } =>      document.set_anchor_untracked(*handle, value),
            AnchorInsert { handle, value } =>   document.insert_anchor_untracked(*handle, value),
            AnchorRemove { handle } =>          document.remove_anchor_untracked(*handle),
            IndentationChange { value } =>      document.set_indentation_untracked(value),
            LanguageChange { value } =>         document.set_language_untracked(&value)
        }
    }
    
}

impl ChangePacket {
    /// Returns a new `ChangePacket` with no changes stored.
    pub fn new() -> ChangePacket {
        ChangePacket {
            changes: vec![]
        }
    }

}

impl UndoRedoStacks {
    /// Returns a new `UndoRedoStacks` with empty stacks and no checkpoint requested.
    pub fn new() -> UndoRedoStacks {
        UndoRedoStacks {
            undo_stack: vec![],
            redo_stack: vec![],
            checkpoint_requested: false
        }
    }
    
    /// Clears the redo stack. This is invoked automatically whenever an undo is
    /// added to the undo stack, but it can be called out of cycle to
    /// invalidate redos by client code.
    pub fn forget_redos(&mut self) -> () {
        if self.redo_stack.len() > 0 {
            self.redo_stack.clear();
        }
    }
    
    /// Clears undos and redos, returning this `UndoRedoStacks` to its
    /// "factory new" configuration. This cannot be undone!
    pub fn forget_everything(&mut self) -> () {
        self.forget_redos();
        
        if self.undo_stack.len() > 0 {
            self.undo_stack.clear();
        }
    }
    
    /// Requests that subsequent actions be added to a new [`ChangePacket`].
    /// This does not immediately add a new change packet, so it can be
    /// called multiple times in quick succession and only one change packet
    /// will be generated.
    /// 
    /// Checkpointing clears the redo stack, regardless. Be advised!
    pub fn checkpoint(&mut self) -> () {
        self.forget_redos();
        self.checkpoint_requested = true;
    }
    
    /// Adds the inverse of a recently applied [`Change`] to the
    /// undo stack, forgetting the redo stack.
    pub fn push_undo(&mut self, change: Change) -> () {
        self.forget_redos();
        
        if self.undo_stack.len() == 0 || self.checkpoint_requested {
            self.undo_stack.push(ChangePacket::new());
        }
        self.checkpoint_requested = false;
        
        self.undo_stack.last_mut().unwrap().changes.push(change);
    }

    /// Returns `(u, r)`, where `u` is the number of undo operations we can perform,
    /// and `r` is the number of redo operations we can perform.
    pub fn depth(&self) -> (usize, usize) {
        (self.undo_stack.len(), self.redo_stack.len())
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
    ///     document.anchor(Anchors::CURSOR).unwrap().position,
    ///     Position { row: 0, column: 0 }
    /// );
    /// assert_eq!(document.undo_redo().depth(), (0, 0));
    /// ```
    pub fn new() -> Document {
        Document {
            lines: vec![Line::from(String::from(""))],
            anchors: Anchors::new(),
            indentation: Indentation::spaces(4),
            undo_redo: UndoRedoStacks::new(),
            language: String::from(""),
            parser: None,
            tree: None
        }
    }

    /// Returns a document initialized from `text`. This sets aside cursor and mark
    /// in the first two anchor indices (cursor at `Anchor::CURSOR`, mark at `Anchor::MARK`)
    /// and initializes them both to (0, 0).
    ///
    /// The resulting document is guaranteed to have at least one line, even if it is
    /// just the empty line.
    ///
    /// # Examples
    ///
    /// ```
    /// use ls_core::document::*;
    /// let empty = Document::from("");
    /// assert_eq!(empty.text(), Document::new().text());
    /// ```
    ///
    /// ```
    /// use ls_core::document::*;
    /// let empty = Document::from("\nHello\n  there!\n");
    /// assert_eq!(*empty.lines(), vec![
    ///     Line::from("".to_string()),
    ///     Line::from("Hello".to_string()),
    ///     Line::from("  there!".to_string()),
    ///     Line::from("".to_string())
    /// ]);
    /// ```
    pub fn from(text: &str) -> Document {
        let lines: Vec<Line> = if text == "" {
            vec![Line::from(String::new())]
        } else {
            util::LINE_SPLIT.split(text).map(|x| Line::from(String::from(x))).collect()
        };

        Document { 
            lines,
            ..Document::new()
        }
    }

    /// Returns a document initialized from `text` with language `language`,
    /// which can be either a file name extension or a string representing the
    /// language's name.
    /// 
    /// See [`Document::from`].
    pub fn from_with_language(text: &str, language: &str) -> Document {
        let mut document = Document::from(text);
        document.set_language_untracked(language);
        document
    }

    /// Returns whether `position` is legal in this document. If a line contains 5
    /// characters, for instance, columns 0 through 5, inclusive, are legal.
    /// 
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\n  there!");
    /// assert_eq!(true, document.position_valid(&Position { row: 0, column: 0 }));
    /// assert_eq!(true, document.position_valid(&Position { row: 0, column: 5 }));
    /// assert_eq!(false, document.position_valid(&Position { row: 0, column: 6 }));
    /// assert_eq!(false, document.position_valid(&Position { row: 2, column: 0 }));
    /// ```
    pub fn position_valid(&self, position: &Position) -> bool {
        position.row < self.lines.len() && position.column <= self.lines[position.row].length
    }

    /// Returns whether `range` is legal in this document. Both its beginning and new and
    /// ending positions must be in range, and its beginning cannot come after its ending.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\n  there!");
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
    pub fn line(&self, index: usize) -> Option<&String> {
        if index >= self.lines.len() {
            None
        } else {
            Some(&self.lines[index].content)
        }
    }

    /// Returns the text of the document as a list of lines. This is guaranteed to contain
    /// at least one line.
    ///
    /// # Examples
    /// ```
    /// use ls_core::document::*;
    /// let document = Document::from("Hello\nthere");
    /// assert_eq!(document.lines()[0].content, "Hello");
    /// assert_eq!(document.lines()[1].content, "there");
    /// ```
    pub fn lines(&self) -> &Vec<Line> {
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
    pub fn anchors(&self) -> hash_map::Iter<'_, AnchorHandle, Anchor> {
        self.anchors.iter()
    }

    /// Returns anchor `handle`, or `None` if invalid handle.
    pub fn anchor(&self, handle: AnchorHandle) -> Option<&Anchor> {
        self.anchors.get(handle)
    }

    /// Returns the cursor.
    pub fn cursor(&self) -> &Anchor {
        self.anchors.cursor()
    }

    /// Returns the mark.
    pub fn mark(&self) -> &Anchor {
        self.anchors.mark()
    }


    /// Returns the [`Range`] representing the region between the cursor and mark.
    /// 
    /// The beginning of the range will be the earlier of the cursor and mark.
    /// There is no way to know whether the start or end of the range is the cursor.
    /// If you need this information, consider using [`Document::cursor`] and
    /// [`Document::mark`] instead.
    pub fn selection(&self) -> Range {
        let cursor = self.cursor().clone();
        let mark = self.mark().clone();
        if cursor.position <= mark.position {
            return Range { beginning: cursor.position, ending: mark.position }
        } else {
            return Range { beginning: mark.position, ending: cursor.position }
        }
    }

    /// Returns the [`UndoRedoStacks`] for this [`Document`].
    pub fn undo_redo(&self) -> &UndoRedoStacks {
        &self.undo_redo
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
        let mut result = String::new();

        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(&line.content);
        }

        result
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
                s.extend(self.lines[range.beginning.row].content.chars()
                        .skip(range.beginning.column)
                        .take(range.ending.column - range.beginning.column));
            } else {
                s.extend(self.lines[range.beginning.row].content.chars()
                        .skip(range.beginning.column));

                for line in self.lines[(range.beginning.row + 1)..range.ending.row].iter() {
                    s += "\n";
                    s += &line.content;
                }

                s += "\n";
                s.extend(self.lines[range.ending.row].content.chars()
                        .take(range.ending.column));
            }

            Some(s)
        }
    }

    /// Returns the parse tree of the document as a `String`, or `None` if
    /// the document could not be parsed. 
    ///
    /// This function does not trigger a parse tree update, but it does perform
    /// expensive string formatting, so do not call it in performance-critical code!
    /// 
    /// The output will appear like this:
    /// ```txt
    /// source_file (0.0 - 0.10) "use hello;"
    ///    use_declaration (0.0 - 0.10) "use hello;"
    ///       use (0.0 - 0.3) "use"
    ///       identifier (0.4 - 0.9) "hello"
    ///       ; (0.9 - 0.10) ";"
    /// ```
    pub fn parse_tree_pretty_print(&self) -> Option<String> {
        match &self.tree {
            None => None,
            Some(tree) => Some(language::pretty_print(&tree.root_node(), self))
        }
    }


    /// Returns a [`Chain`] of [`ChainRegion`]s encompassing the given `position`
    /// in this document, or an [`Oops`] if either the position is invalid
    /// or this document has no parse tree.
    /// 
    /// This can be used to determine what nested structures surround
    /// a certain position.
    pub fn get_context_at(&self, position: &Position) -> Result<Chain, Oops> {
        if !self.position_valid(position) {
            return Err(Oops::InvalidPosition(position.clone(), "get_context_at"));
        }
        
        if let None = self.tree {
            return Err(Oops::CannotParse("get_context_at"));
        }
        
        let b = util::cp_index_to_byte(&self.lines[position.row].content, position.column).unwrap();
        let pt = tree_sitter::Point::new(position.row, b);
        
        let mut chain = Chain::new();
        let mut node = self.tree.as_ref().unwrap().root_node();
        
        'outer: loop {
            chain.push(node.kind(), node.range(), self);
            
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                let child_range = child.range();
                if child_range.start_point <= pt && pt <= child_range.end_point {
                    node = child;
                    continue 'outer;
                }
            }
            
            break;
        }
        
        Ok(chain)
    }

    /// Returs a `Vec<String>` prepared for insertion from `text`, a `&str`,
    /// under insert options `options` at `position`.
    #[allow(unused_variables)]
    fn prep_text(text: &str, position: &Position, options: &InsertOptions) -> Vec<String> {
        if options.spacing || options.escapes || options.indent {
            todo!();
        }
        
        let mut lines: Vec<String> = vec![];
        
        for line in util::LINE_SPLIT.split(text) {
            lines.push(String::from(line));
        }
        
        lines
    }
    
    /// Inserts `text` into the document with `options`.
    pub fn insert(&mut self, text: &str, options: &InsertOptions) -> Result<(), Oops> {
        let range = match options.range {
            None => self.selection(),
            Some(r) => {
                if !self.range_valid(&r) {
                    return Err(Oops::InvalidRange(r, "insert"));
                }
                r
            }
        };

        if !range.empty() {
            if let Err(oops) = self.remove(&RemoveOptions::exact_at(&range)) {
                return Err(oops);
            }
        }

        let lines = Self::prep_text(text, &range.beginning, options);

        if lines.len() == 0 || (lines.len() == 1 && lines[0].len() == 0) {
            return Err(Oops::EmptyString("can't insert nothing"));
        }
     
        let mut anchor_changes: Vec<Change> = vec![];

        for (handle, anchor) in self.anchors.iter() {
            if anchor.position >= range.beginning {
                let mut moved = anchor.clone();

                if moved.position.row == range.beginning.row {
                    if lines.len() == 1 {
                        moved.position.column += lines[0].chars().count();
                    } else {
                        let past_original = if moved.position.column > range.beginning.column {
                            moved.position.column - range.beginning.column
                        } else {
                            0
                        };
                        
                        moved.position.column = lines[lines.len() - 1].chars().count() + past_original;
                    }
                }

                moved.position.row += lines.len() - 1;

                anchor_changes.push(Change::AnchorSet {
                    handle: *handle,
                    value: moved
                });
            }
        }

        
        let inverse = Change::Insert {
            text: lines,
            position: range.beginning
        }.apply_untracked(self);
        self.undo_redo.push_undo(inverse);

        for change in anchor_changes {
            let inverse = change.apply_untracked(self);
            self.undo_redo.push_undo(inverse);
        }
        
        Ok(())
    }


    /// Removes the current selection (or the range specified in `options`).
    pub fn remove(&mut self, options: &RemoveOptions) -> Result<(), Oops> {
        let range = match options.range {
            None => self.selection(),
            Some(r) => {
                if !self.range_valid(&r) {
                    return Err(Oops::InvalidRange(r, "remove"));
                }
                r
            }
        };

        if range.empty() {
            return Err(Oops::InvalidRange(range, "remove - empty"));
        }

        let mut anchor_changes: Vec<Change> = vec![];

        for (handle, anchor) in self.anchors.iter() {
            if anchor.position > range.ending {
                anchor_changes.push(Change::AnchorSet { 
                    handle: *handle,
                    value: Anchor {
                        position: Position::from(
                            anchor.position.row - (range.ending.row - range.beginning.row),
                            if anchor.position.row == range.ending.row {
                                range.beginning.column + anchor.position.column - range.ending.column
                            } else {
                                anchor.position.column
                            }
                        ),
                        ..*anchor
                    }
                });
            } else if anchor.position > range.beginning {
                anchor_changes.push(Change::AnchorSet {
                    handle: *handle,
                    value: Anchor {
                        position: range.beginning,
                        ..*anchor
                    }
                });
            }
        }

        
        let inverse = Change::Remove {
            range
        }.apply_untracked(self);
        self.undo_redo.push_undo(inverse);

        for change in anchor_changes {
            let inverse = change.apply_untracked(self);
            self.undo_redo.push_undo(inverse);
        }

        Ok(())
    }

    
    
    /// Sets anchor `handle` to `value`. Returns an `Err` if `handle` does not
    /// exist or if `value` points to an invalid position.
    pub fn set_anchor(&mut self, handle: AnchorHandle, value: &Anchor) -> Result<(), Oops> {
        if let None = self.anchors.get(handle) {
            return Err(Oops::NonexistentAnchor(handle));
        }
        if !self.position_valid(&value.position) {
            return Err(Oops::InvalidPosition(value.position, "set_anchor"));
        }

        let inverse = self.set_anchor_untracked(handle, value);
        self.undo_redo.push_undo(inverse);

        Ok(())
    }
    
    /// Creates a new anchor with contents `anchor`, returning its
    /// [`AnchorHandle`] or `Err` if the requested position is invalid.
    pub fn create_anchor(&mut self, anchor: &Anchor) -> Result<AnchorHandle, Oops> {
        if !self.position_valid(&anchor.position) {
            return Err(Oops::InvalidPosition(anchor.position, "create_anchor"));
        }

        let handle = self.anchors.get_new_handle();
        let inverse = self.insert_anchor_untracked(handle, anchor);
        self.undo_redo.push_undo(inverse);

        Ok(handle)
    }
    
    /// Moves the cursor to `position`.
    pub fn set_cursor(&mut self, position: &Position) -> Result<(), Oops> {
        self.set_anchor(Anchors::CURSOR, &Anchor {
            position: *position,
            ..*self.anchors.get(Anchors::CURSOR).unwrap()
        })
    }
    
    /// Moves the mark to `position`.
    pub fn set_mark(&mut self, position: &Position) -> Result<(), Oops> {
        self.set_anchor(Anchors::MARK, &Anchor {
            position: *position,
            ..*self.anchors.get(Anchors::MARK).unwrap()
        })
    }
    
    /// Moves both cursor and mark to `position`.
    pub fn set_cursor_and_mark(&mut self, position: &Position) -> Result<(), Oops> {
        self.set_cursor(position)?;
        self.set_mark(position)?;
        Ok(())
    }
    
    /// Moves the mark to the beginning of `range` and the cursor to the 
    /// end of `range`.
    pub fn set_selection(&mut self, range: &Range) -> Result<(), Oops> {
        if !self.range_valid(range) {
            Err(Oops::InvalidRange(*range, "set_selection"))
        } else {
            self.set_mark(&range.beginning)?;
            self.set_cursor(&range.ending)?;
            Ok(())
        }
    }
    
    /// Removes the anchor at `handle`, or returns `Err` if invalid.
    pub fn remove_anchor(&mut self, handle: AnchorHandle) -> Result<(), Oops> {
        if let None = self.anchors.get(handle) {
            return Err(Oops::NonexistentAnchor(handle));
        }

        let inverse = self.remove_anchor_untracked(handle);

        self.undo_redo.push_undo(inverse);
        Ok(())
    }
    
    /// Sets the indentation policy of this document to `indentation`.
    /// Does not actually change the document's text!
    pub fn set_indentation(&mut self, indentation: &Indentation) -> Result<(), Oops> {
        let inverse = self.set_indentation_untracked(indentation);
        self.undo_redo.push_undo(inverse);
        Ok(())
    }

    /// Sets the language of this document to `language` and rebuilds the parse tree.
    pub fn set_language(&mut self, language: &str) -> Result<(), Oops> {
        let inverse = self.set_language_untracked(language);
        self.undo_redo.push_undo(inverse);
        Ok(())
    }

    /// Update the parse tree for this document, acquiring a new parser if necessary.
    /// This function will never fail, but might leave the document with no parse tree.
    pub fn update_parse(&mut self) -> () {
        if self.parser.is_none() {
            self.parser = language::get_parser(&self.language);
            if self.parser.is_none() {
                self.tree = None;
                return ();
            }
        }
        
        // At this point, we have a parser. We just need to update the tree
        let text = self.text();
        if let Some(p) = &mut self.parser {
            let new_tree = p.parse(text, None); /*match &self.tree {
                None => None,
                Some(tree) => Some(&tree)
            });*/
            self.tree = new_tree;
        }
        
        ()
    }

    /// Undoes the most recently performed [`ChangePacket`], or returns error
    /// if there is nothing to undo.
    pub fn undo_once(&mut self) -> Result<(), Oops> {
        match self.undo_redo.undo_stack.pop() {
            None => Err(Oops::NoMoreUndos(0)),
            Some(packet) => {
                let mut redo_packet = ChangePacket::new();
                for inverse in packet.changes.iter().rev() {
                    redo_packet.changes.push(inverse.apply_untracked(self));
                }
                
                self.undo_redo.redo_stack.push(redo_packet);
                Ok(())
            }
        }
    }

    /// Undoes `quantity` [`ChangePacket`]s.
    /// 
    /// Returns `Ok(times)` or `Oops::NoMoreUndos(times)`,
    /// where `times` is the number of change packets undone.
    pub fn undo(&mut self, quantity: usize) -> Result<usize, Oops> {
        for times in 0..quantity {
            let result = self.undo_once();
            match result {
                Ok(_) => (),
                Err(_) => return Err(Oops::NoMoreUndos(times))
            }
        }

        Ok(quantity)
    }
    
    /// Redoes the most recently undone [`ChangePacket`], or returns error
    /// if there is nothing to redo.
    pub fn redo_once(&mut self) -> Result<(), Oops> {
        match self.undo_redo.redo_stack.pop() {
            None => Err(Oops::NoMoreRedos(0)),
            Some(packet) => {
                let mut undo_packet = ChangePacket::new();
                for inverse in packet.changes.iter().rev() {
                    undo_packet.changes.push(inverse.apply_untracked(self));
                }
                
                self.undo_redo.undo_stack.push(undo_packet);
                Ok(())
            }
        }
    }


    /// Redoes `quantity` [`ChangePacket`]s.
    /// 
    /// Returns `Ok(times)` or `Oops::NoMoreRedos(times)`,
    /// where `times` is the number of change packets redone.
    pub fn redo(&mut self, quantity: usize) -> Result<usize, Oops> {
        for times in 0..quantity {
            let result = self.redo_once();
            match result {
                Ok(_) => (),
                Err(_) => return Err(Oops::NoMoreRedos(times))
            }
        }

        Ok(quantity)
    }

    /// Requests a checkpoint from the [`UndoRedoStacks`]. This means that
    /// the next undoable operation will occur on its own [`ChangePacket`].
    pub fn checkpoint(&mut self) -> () {
        self.undo_redo.checkpoint();
    }
    
    /// Forgets all undo and redo data, meaning that the current state
    /// of the document becomes the start of history.  Use wisely!
    pub fn forget_undo_redo(&mut self) -> Result<(), Oops> {
        self.undo_redo.forget_everything();
        Ok(())
    }
    





    
    /// Inserts `text`, a list of one or more lines, into the document at `position`.
    /// Returns the `Change` which would undo this modification.
    /// 
    /// This does not process escapes, indentation, spacing, or capitalization.
    /// The *only* thing it does is insert exactly what it is told to.
    ///
    /// # Panics
    /// Panics if asked to insert 0 lines or if `position` is out of range.
    #[allow(unused_assignments)]
    fn insert_untracked(&mut self, text: &Vec<String>, position: &Position) -> Change {
        if text.len() == 0 {
            panic!("cannot insert 0 lines");
        }
        self.assert_position_valid(position);

        let after = self.lines[position.row].content.chars().skip(position.column).collect::<String>();
        let before = self.lines[position.row].content.chars().take(position.column).collect::<String>();
        let mut col = 0;

        if text.len() == 1 {
            self.lines[position.row].content = before + &text[0];
            col = self.lines[position.row].content.chars().count();

            self.lines[position.row].content += &after;
            self.lines[position.row].length = self.lines[position.row].content.chars().count();
        } else {
            self.lines[position.row].content = before + &text[0];
            self.lines[position.row].length = self.lines[position.row].content.chars().count();

            let to_append = text.into_iter().skip(1).map(|x| Line::from(x.clone())).collect::<Vec<Line>>();
            
            push_all_at(&mut self.lines, position.row + 1, &to_append);

            col = self.lines[position.row + text.len() - 1].length;
            self.lines[position.row + text.len() - 1].content += &after;
            self.lines[position.row + text.len() - 1].length += after.chars().count();
        }

        self.update_parse();

        Change::Remove { range: Range {
            beginning: *position,
            ending: Position { 
                row: position.row + text.len() - 1,
                column: col
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
            let original = substring(&self.lines[range.beginning.row].content,
                range.beginning.column, range.ending.column - range.beginning.column
            ).to_string();

            self.lines[range.beginning.row] = Line::from(
                slice(&self.lines[range.beginning.row].content,
                    ..range.beginning.column
                ).to_string() +
                &slice(&self.lines[range.beginning.row].content,
                    range.ending.column..
                )
            );

            self.update_parse();

            Change::Insert {
                text: vec![original],
                position: range.beginning
            }
        } else {
            let mut lines: Vec<String> = Vec::new();

            lines.push(
                slice(&self.lines[range.beginning.row].content, range.beginning.column..).to_string()
            );

            self.lines[range.beginning.row].content = substring(
                &self.lines[range.beginning.row].content,
                0, range.beginning.column
            ).to_string();

            let trailing = slice(&self.lines[range.ending.row].content, range.ending.column..)
                .to_string();

            self.lines[range.ending.row].content = substring(
                &self.lines[range.ending.row].content, 0, range.ending.column
            ).to_string();

            self.lines[range.beginning.row].content += &trailing;
            self.lines[range.beginning.row].length = 
                self.lines[range.beginning.row].content.chars().count();

            lines.extend(
                self.lines
                    .drain((range.beginning.row + 1)..= range.ending.row)
                    .map(|x| x.content)
            );

            self.update_parse();

            Change::Insert {
                text: lines,
                position: range.beginning
            }
        }
    }
    
    /// Sets the content of anchor `handle` to `value`.
    /// Returns the `Change` which would undo this modification.
    fn set_anchor_untracked(&mut self, handle: AnchorHandle, value: &Anchor) -> Change {
        match self.anchors.set(handle, value) {
            Err(_) => panic!("Tried to set invalid anchor handle {}", handle),
            Ok(original) => Change::AnchorSet { handle, value: original }
        }
    }
    
    /// Inserts a new anchor at `handle` with value `value`.
    /// Returns the `Change` which would undo this modification.
    fn insert_anchor_untracked(&mut self, handle: AnchorHandle, value: &Anchor) -> Change {
        self.anchors.create(*value, Some(handle));

        Change::AnchorRemove { handle }
    }
    
    /// Removes the anchor at `handle`.
    /// Returns the `Change` which would undo this modification.
    fn remove_anchor_untracked(&mut self, handle: AnchorHandle) -> Change {
        match self.anchors.remove(handle) {
            Ok(old) => Change::AnchorInsert { handle, value: old },
            Err(_) => {
                panic!("Tried to remove nonexistent anchor handle {}", handle)
            }
        }
    }

    /// Sets the indentation policy.
    fn set_indentation_untracked(&mut self, value: &Indentation) -> Change {
        let reverse = Change::IndentationChange { value: self.indentation };
        self.indentation = *value;
        
        reverse
    }

    /// Sets the language string for this document, rebuilding the current parse tree
    /// under the new language.
    fn set_language_untracked(&mut self, language: &str) -> Change {
        let reverse = Change::LanguageChange { value: String::from(&self.language) };
        self.language = String::from(language);
        self.parser = None;
        self.tree = None;
        self.update_parse();
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
        let inverse = document.set_anchor_untracked(Anchors::CURSOR, &Anchor {
            position: Position { row: 1, column: 3 }
        });

        assert_eq!(document.cursor().position, Position { row: 1, column: 3 });

        assert_eq!(inverse, Change::AnchorSet {
            handle: Anchors::CURSOR,
            value: Anchor {
                position: Position { row: 0, column: 0 }
            }
        });
    }

    #[test]
    fn insert_remove_anchor_untracked() {
        let mut document = Document::from("AAA\nBBB");
        let inverse = document.insert_anchor_untracked(2, &Anchor {
            position: Position { row: 1, column: 3 }
        });

        assert_eq!(document.anchor(2).unwrap().position, Position { row: 1, column: 3 });
        assert_eq!(inverse, Change::AnchorRemove { handle: 2 });

        let inverse_2 = inverse.apply_untracked(&mut document);

        assert_eq!(document.anchors().len(), 2);
        assert_eq!(inverse_2, Change::AnchorInsert {
            handle: 2,
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
        assert_eq!(document.lines()[0].content, "🙈我爱unicode🦄");
        assert_eq!(document.lines()[1].content, "매우 짜증나");
        assert_eq!(document.lines()[0].length, 11);
        assert_eq!(document.lines()[1].length, 6);
        
        assert_eq!(document.text(), "🙈我爱unicode🦄\n매우 짜증나");

        let chg = document.insert_untracked(&vec![
            "👋🏻🤚🏻🖐🏻✋🏻🖖🏻👌🏻".chars().collect(),
            "⌚️📱📲💻⌨️".chars().collect(),
            "".chars().collect()
        ], &Position::from(1, 0));
        assert_eq!(document.text(), "🙈我爱unicode🦄\n👋🏻🤚🏻🖐🏻✋🏻🖖🏻👌🏻\n⌚️📱📲💻⌨️\n매우 짜증나");
        assert_eq!(document.lines()[0].length, 11);
        assert_eq!(document.lines()[1].length, 12);
        assert_eq!(document.lines()[2].length, 7);
        assert_eq!(document.lines()[3].length, 6);
        
        // Some emojis are two codepoints in a row...
        // We don't handle that. Nope.
        // (1, 6) is just after 👋🏻🤚🏻🖐🏻
        // (2, 3) is just after ⌚️📱
        let chg_2 = document.remove_untracked(&Range::from(1, 6, 2, 3));
        assert_eq!(document.text(), "🙈我爱unicode🦄\n👋🏻🤚🏻🖐🏻📲💻⌨️\n매우 짜증나");
        assert_eq!(document.lines()[0].length, 11);
        assert_eq!(document.lines()[1].length, 10);
        assert_eq!(document.lines()[2].length, 6);
        
        chg_2.apply_untracked(&mut document);
        assert_eq!(document.text(), "🙈我爱unicode🦄\n👋🏻🤚🏻🖐🏻✋🏻🖖🏻👌🏻\n⌚️📱📲💻⌨️\n매우 짜증나");

        chg.apply_untracked(&mut document);
        assert_eq!(document.text(), "🙈我爱unicode🦄\n매우 짜증나");
        assert_eq!(document.lines()[0].length, 11);
        assert_eq!(document.lines()[1].length, 6);
        
    }

    #[test]
    fn remove_untracked() {
        let mut document = Document::from("01234\nabcde\nABCDE");

        assert_eq!(
            document.remove_untracked(&Range::from(1, 2, 1, 2)),
            Change::Insert {
                text: vec!["".to_string()],
                position: Position::from(1, 2)
            }
        );
        assert_eq!(document.text(), "01234\nabcde\nABCDE");

        assert_eq!(
            document.remove_untracked(&Range::from(1, 2, 1, 4)),
            Change::Insert {
                text: vec!["cd".to_string()],
                position: Position::from(1, 2)
            }
        );
        assert_eq!(document.text(), "01234\nabe\nABCDE");

        assert_eq!(
            document.remove_untracked(&Range::from(0, 4, 1, 1)),
            Change::Insert {
                text: vec!["4".to_string(), "a".to_string()],
                position: Position::from(0, 4)
            }
        );
        assert_eq!(document.text(), "0123be\nABCDE");
    }

    #[test]
    fn insert_remove_undo_redo() {
        let mut document = Document::from("");

        document.insert("Hello", &InsertOptions::exact()).unwrap();
        assert_eq!(document.text(), "Hello");
        assert_eq!(document.undo_redo().depth(), (1, 0));
        assert_eq!(document.cursor().position, Position::from(0, 5));
        assert_eq!(document.mark().position, Position::from(0, 5));

        document.undo_redo.checkpoint();
        document.insert("\nthere\ncaptain", &InsertOptions::exact()).unwrap();
        assert_eq!(document.text(), "Hello\nthere\ncaptain");
        assert_eq!(document.undo_redo().depth(), (2, 0));
        assert_eq!(document.cursor().position, Position::from(2, 7));
        assert_eq!(document.mark().position, Position::from(2, 7));
        
        assert_eq!(document.undo(1).unwrap(), 1);
        assert_eq!(document.text(), "Hello");
        assert_eq!(document.undo_redo().depth(), (1, 1));
        assert_eq!(document.cursor().position, Position::from(0, 5));
        assert_eq!(document.mark().position, Position::from(0, 5));

        assert_eq!(document.undo(1).unwrap(), 1);
        assert_eq!(document.text(), "");
        assert_eq!(document.undo_redo().depth(), (0, 2));
        assert_eq!(document.cursor().position, Position::from(0, 0));
        assert_eq!(document.mark().position, Position::from(0, 0));

        assert_eq!(document.undo(1).unwrap_err(), Oops::NoMoreUndos(0));

        assert_eq!(document.undo_redo().depth(), (0, 2));
        assert_eq!(document.redo(100).unwrap_err(), Oops::NoMoreRedos(2));
        assert_eq!(document.undo_redo().depth(), (2, 0));
        assert_eq!(document.text(), "Hello\nthere\ncaptain");
        assert_eq!(document.undo_redo().depth(), (2, 0));
        assert_eq!(document.cursor().position, Position::from(2, 7));
        assert_eq!(document.mark().position, Position::from(2, 7));
        
        document.checkpoint();
        document.remove(&RemoveOptions::exact_at(&Range::from(0, 2, 2, 1))).unwrap();
        assert_eq!(document.undo_redo().depth(), (3, 0));
        assert_eq!(document.text(), "Heaptain");
        assert_eq!(document.cursor().position, Position::from(0, 8));
        assert_eq!(document.mark().position, Position::from(0, 8));
        
        assert_eq!(document.undo(1).unwrap(), 1);
        assert_eq!(document.text(), "Hello\nthere\ncaptain");
        assert_eq!(document.cursor().position, Position::from(2, 7));

        document.insert("ooo", &InsertOptions::exact_at(&Range::from(1, 1, 2, 3))).unwrap();
        assert_eq!(document.text(), "Hello\ntoootain");
        assert_eq!(document.undo_redo().depth(), (2, 0));
        assert_eq!(document.cursor().position, Position::from(1, 8));

        document.forget_undo_redo().unwrap();
        assert_eq!(document.undo_redo().depth(), (0, 0));
    }

    #[test]
    fn anchors() {
        let mut document = Document::from_with_language("🙈火A\n日BB\nCC魔", "rs");
        
        let a = document.create_anchor(&Anchor::from(0, 0)).unwrap();
        let b = document.create_anchor(&Anchor::from(0, 2)).unwrap();
        let c = document.create_anchor(&Anchor::from(1, 1)).unwrap();
        let d = document.create_anchor(&Anchor::from(1, 3)).unwrap();
        let e = document.create_anchor(&Anchor::from(2, 0)).unwrap();
        let f = document.create_anchor(&Anchor::from(2, 2)).unwrap();
        document.insert("Hello\nThere", &InsertOptions::exact_at(&Range::from(1, 0, 1, 0))).unwrap();

        document.checkpoint();
        assert_eq!(document.text(), "🙈火A\nHello\nThere日BB\nCC魔");
        assert_eq!(document.anchor(a).unwrap().position, Position::from(0, 0));
        assert_eq!(document.anchor(b).unwrap().position, Position::from(0, 2));
        assert_eq!(document.anchor(c).unwrap().position, Position::from(2, 6));
        assert_eq!(document.anchor(d).unwrap().position, Position::from(2, 8));
        assert_eq!(document.anchor(e).unwrap().position, Position::from(3, 0));
        assert_eq!(document.anchor(f).unwrap().position, Position::from(3, 2));

        assert_eq!(document.indentation, Indentation::spaces(4));
        document.set_indentation(&Indentation::tabs(2)).unwrap();
        assert_eq!(document.indentation, Indentation::tabs(2));

        document.remove(&RemoveOptions::exact_at(&Range::from(2, 5, 2, 6))).unwrap();
        assert_eq!(document.text(), "🙈火A\nHello\nThereBB\nCC魔");
        assert_eq!(document.anchor(a).unwrap().position, Position::from(0, 0));
        assert_eq!(document.anchor(b).unwrap().position, Position::from(0, 2));
        assert_eq!(document.anchor(c).unwrap().position, Position::from(2, 5));
        assert_eq!(document.anchor(d).unwrap().position, Position::from(2, 7));
        assert_eq!(document.anchor(e).unwrap().position, Position::from(3, 0));
        assert_eq!(document.anchor(f).unwrap().position, Position::from(3, 2));
        
        document.remove(&RemoveOptions::exact_at(&Range::from(0, 1, 1, 0))).unwrap();
        document.remove_anchor(a).unwrap();

        assert_eq!(document.text(), "🙈Hello\nThereBB\nCC魔");
        assert_eq!(document.anchor(b).unwrap().position, Position::from(0, 1));
        assert_eq!(document.anchor(c).unwrap().position, Position::from(1, 5));
        assert_eq!(document.anchor(d).unwrap().position, Position::from(1, 7));
        assert_eq!(document.anchor(e).unwrap().position, Position::from(2, 0));
        assert_eq!(document.anchor(f).unwrap().position, Position::from(2, 2));
        
        document.remove(&RemoveOptions::exact_at(&Range::from(1, 5, 2, 1))).unwrap();
        assert_eq!(document.text(), "🙈Hello\nThereC魔");
        assert_eq!(document.anchor(b).unwrap().position, Position::from(0, 1));
        assert_eq!(document.anchor(c).unwrap().position, Position::from(1, 5));
        assert_eq!(document.anchor(d).unwrap().position, Position::from(1, 5));
        assert_eq!(document.anchor(e).unwrap().position, Position::from(1, 5));
        assert_eq!(document.anchor(f).unwrap().position, Position::from(1, 6));
        
        
        document.undo(1).unwrap();
        assert_eq!(document.undo_redo().depth(), (1, 1));
        assert_eq!(document.text(), "🙈火A\nHello\nThere日BB\nCC魔");
        assert_eq!(document.anchor(a).unwrap().position, Position::from(0, 0));
        assert_eq!(document.anchor(b).unwrap().position, Position::from(0, 2));
        assert_eq!(document.anchor(c).unwrap().position, Position::from(2, 6));
        assert_eq!(document.anchor(d).unwrap().position, Position::from(2, 8));
        assert_eq!(document.anchor(e).unwrap().position, Position::from(3, 0));
        assert_eq!(document.anchor(f).unwrap().position, Position::from(3, 2));

        assert_eq!(document.indentation, Indentation::spaces(4));
    }

    #[test]
    fn parsing() {
        let mut document = Document::from_with_language("use hello;", "rs");
        assert_eq!(
            document.parse_tree_pretty_print().unwrap(),
r#"source_file (0.0 - 0.10) "use hello;"
   use_declaration (0.0 - 0.10) "use hello;"
      use (0.0 - 0.3) "use"
      identifier (0.4 - 0.9) "hello"
      ; (0.9 - 0.10) ";"
"#);

        document.checkpoint();
        document.set_cursor_and_mark(&Position::from(0, 9)).unwrap();
        document.insert("::world", &InsertOptions::exact()).unwrap();

        assert_eq!(
            document.parse_tree_pretty_print().unwrap(),
r#"source_file (0.0 - 0.17) "use hello::world;"
   use_declaration (0.0 - 0.17) "use hello::world;"
      use (0.0 - 0.3) "use"
      scoped_identifier (0.4 - 0.16) "hello::world"
         identifier (0.4 - 0.9) "hello"
         :: (0.9 - 0.11) "::"
         identifier (0.11 - 0.16) "world"
      ; (0.16 - 0.17) ";"
"#);

        document.undo(1).unwrap();
        assert_eq!(
            document.parse_tree_pretty_print().unwrap(),
r#"source_file (0.0 - 0.10) "use hello;"
   use_declaration (0.0 - 0.10) "use hello;"
      use (0.0 - 0.3) "use"
      identifier (0.4 - 0.9) "hello"
      ; (0.9 - 0.10) ";"
"#);

        document.checkpoint();
        document.set_language("js").unwrap();
        assert_eq!(
            document.parse_tree_pretty_print().unwrap(),
r#"program (0.0 - 0.10) "use hello;"
   ERROR (0.0 - 0.3) "use"
      identifier (0.0 - 0.3) "use"
   expression_statement (0.4 - 0.10) "hello;"
      identifier (0.4 - 0.9) "hello"
      ; (0.9 - 0.10) ";"
"#);
        
        document.undo(1).unwrap();
        assert_eq!(
            document.parse_tree_pretty_print().unwrap(),
r#"source_file (0.0 - 0.10) "use hello;"
   use_declaration (0.0 - 0.10) "use hello;"
      use (0.0 - 0.3) "use"
      identifier (0.4 - 0.9) "hello"
      ; (0.9 - 0.10) ";"
"#);
    }

    #[test]
    fn chains() {
        let document = Document::from_with_language(
r#"
pub fn isPrime(x: u32) -> bool { 
    for k in 2..x {
        if x % k == 0 {
            return false;
        }
    }
    true
}
"#,
            "rs"
        );

        assert_eq!(
            &format!("{}", document.get_context_at(&Position::from(9, 0)).unwrap()),
r#"source_file (1, 0)-(9, 0)
"#
        );

        assert_eq!(
            &format!("{}", document.get_context_at(&Position::from(4, 15)).unwrap()),
r#"source_file (1, 0)-(9, 0)
function_item (1, 0)-(8, 1)
block (1, 31)-(8, 1)
for_expression (2, 4)-(6, 5)
block (2, 18)-(6, 5)
if_expression (3, 8)-(5, 9)
block (3, 22)-(5, 9)
return_expression (4, 12)-(4, 24)
return (4, 12)-(4, 18)
"#
        );

        assert_eq!(
            &format!("{}", document.get_context_at(&Position::from(1, 21)).unwrap()),
r#"source_file (1, 0)-(9, 0)
function_item (1, 0)-(8, 1)
parameters (1, 14)-(1, 22)
parameter (1, 15)-(1, 21)
primitive_type (1, 18)-(1, 21)
"#
        );
    }
}
