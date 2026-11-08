# Oraculum

A terminal tool for solo and GMless tabletop RPG play: it answers yes/no questions, rolls dice, and rolls on tables and generators, logging everything to a journal.

## Language

### Rolling

**Oracle**:
The yes/no question mechanism: a d100 against a likelihood gives an answer.
_Avoid_: Fate roll, yes/no roll

**Likelihood**:
How probable the user judges a yes answer, one of five labels from Very unlikely to Very likely.
_Avoid_: Odds, difficulty

**Exceptional**:
An answer in the outer fifth of its side, either Exceptional Yes or Exceptional No.
_Avoid_: Critical, extreme

**Twist**:
The signal that a question's roll was a double (11, 22, ... 99), meaning an Event is shown beside the answer.
_Avoid_: Random event, complication

**Event**:
The content generated when a Twist occurs.
_Avoid_: Twist (that is the signal, not the content)

**Dice expression**:
Text such as `3d20kl1+2` that describes a roll, including keep-highest and keep-lowest.
_Avoid_: Formula, dice string

### Content

**Table**:
A list of text entries chosen by a dice roll, either ranged or a plain list.
_Avoid_: Chart, list (unless the plain-list form is meant)

**Generator**:
A recipe that builds a result by combining several tables or other generators into a template.
_Avoid_: Procedure, builder

**Slot**:
One named part of a Generator, filled from a table, another generator, or a dice expression.
_Avoid_: Field, placeholder (a placeholder is the marker in the template)

**Item**:
Either a Table or a Generator.
_Avoid_: Entry (an entry is one row of a Table), asset

**Entry**:
One row of a Table.
_Avoid_: Item, row

**Built-in**:
Content shipped with Oraculum.
_Avoid_: Default, stock

**User content**:
Tables and generators the user adds as files, which can replace a Built-in of the same kind and id.
_Avoid_: Custom, mod

**Catalog**:
The complete set of Items loaded and validated for a run.
_Avoid_: Registry, index

**Library**:
The screen where the user browses and rolls every Item in the Catalog.
_Avoid_: Browser, catalog (the Catalog is the loaded set, the Library is the screen)

### Records

**Session**:
A named series of play whose rolls are kept together.
_Avoid_: Campaign, game

**Journal**:
The record of everything rolled in a Session, plus the user's notes.
_Avoid_: Log, history

**Journal entry**:
One recorded roll or note in the Journal.
_Avoid_: Record, line

**Note**:
Free text the user adds to the Journal.
_Avoid_: Comment, memo

**Seed**:
The number that makes every roll in a run reproducible.
_Avoid_: Random key
