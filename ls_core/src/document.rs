//! A buffer of text organized into lines. Equipped with undo, redo, and anchors.
//!
//! Supports advanced language features, parsing, and many other useful features
//! that enable speech coding.




//-----------------------------------------------------------------------------

/// A row-column position in a [`Document`].
/// 
/// Positions are indexed from 0. Tabs count for 1 character. For this reason,
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

    /// Represents inserting `text` at `position`.
    Insert { text: String, position: Position },

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
    AnchorRemove { index: usize }
}

/// A series of [`Change`] to be applied as a group.
/// 
/// Because individual changes are typically rather small atoms, user actions
/// (e.g. pressing Ctrl-Z) undo entire [`ChangePacket`]s. 
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ChangePacket {
    changes: Vec<Change>
}

/// A buffer of text organized into lines. Equipped with undo, redo, and anchors.
/// The top-level struct for this module.
///
/// The [`Document`] is central to ls_core. Clients of ls_core are likely
/// to spend much of their time working with this type.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Document {
    lines: Vec<String>,
    anchors: Vec<Anchor>,
    undo_buffer: Vec<ChangePacket>,
    redo_buffer: Vec<ChangePacket>
}



//-----------------------------------------------------------------------------

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
    fn apply(&self, document: &mut Document) -> Change {
        use Change::*;
    
        match self {
            Insert { text, position } =>        document.insert_untracked(&text, position),
            Remove { range } =>                 document.remove_untracked(range),
            AnchorSet { index, value } =>       document.set_anchor_untracked(*index, value),
            AnchorInsert { index, value } =>    document.insert_anchor_untracked(*index, value),
            AnchorRemove { index } =>           document.remove_anchor_untracked(*index)
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
    /// assert_eq!(*document.lines(), vec![String::from("")]);
    /// assert_eq!(document.anchors().len(), 2);
    /// assert_eq!(
    ///     document.anchor(Anchor::CURSOR).unwrap().position,
    ///     Position { row: 0, column: 0 }
    /// );
    /// assert_eq!(document.undo_redo_depth(), (0, 0));
    /// ```
    pub fn new() -> Document {
        Document {
            lines: vec![String::new()],
            anchors: vec![Anchor::default(), Anchor::default()],
            undo_buffer: Vec::new(),
            redo_buffer: Vec::new()
        }
    }

    /// Returns a document initialized from `text`. This sets aside cursor and mark
    /// in the first two anchor indices (cursor at `Anchor::CURSOR`, mark at `Anchor::MARK`)
    /// and initializes them both to (0, 0).
    ///
    /// The resulting document is guaranteed to have at least one line, even if it is
    /// just the empty line. Trailing newlines are stripped and the final empty line
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
    ///     String::from("Hello"),
    ///     String::from("  there!")
    /// ]);
    /// ```
    pub fn from(text: &str) -> Document {
        let lines: Vec<String> = if text == "" {
            vec![String::new()]
        } else {
            text.lines().map(|x| String::from(x)).collect()
        };

        Document {
            lines,
            anchors: vec![Anchor::default(), Anchor::default()],
            undo_buffer: Vec::new(),
            redo_buffer: Vec::new()
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

    /// Returns the text of the document as a list of lines. This is guaranteed to contain
    /// at least one line.
    pub fn lines(&self) -> &Vec<String> {
        &self.lines
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




    fn insert_untracked(&mut self, text: &str, position: &Position) -> Change {
        todo!();
    }
    
    fn remove_untracked(&mut self, range: &Range) -> Change {
        todo!();
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
    /// Panics if index is out of range or if the anchor points to an invalid position.
    fn insert_anchor_untracked(&mut self, index: usize, value: &Anchor) -> Change {
        if index < 2 || index > self.anchors.len() {
            panic!("insert_anchor_untracked: invalid index {}", index);
        }
        
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
    /// Panics if index is out of range or if the anchor points to an invalid position.
    fn remove_anchor_untracked(&mut self, index: usize) -> Change {
        if index < 2 || index >= self.anchors.len() {
            panic!("remove_anchor_untracked: invalid index {}", index);
        }
        
        let orig_value = self.anchors[index];
        self.anchors.remove(index);
        
        Change::AnchorInsert { index, value: orig_value }     
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

        let inverse_2 = inverse.apply(&mut document);

        assert_eq!(document.anchors().len(), 2);
        assert_eq!(inverse_2, Change::AnchorInsert {
            index: 2,
            value: Anchor {
                position: Position { row: 1, column: 3 }
            }
        });
    }
}