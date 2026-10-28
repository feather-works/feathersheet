# FeatherSheet roadmap — up to 1.0 (100% functional)

This roadmap gathers, in one place, everything still missing for FeatherSheet
to be **100% functional**: every command an Excel user looks for exists and
works, every common formula calculates the same way, every `.xlsx` file opens
and saves without losing anything, and everyday macros run.

The numbers come from the detailed surveys (current as of v0.2.0, 10/27/2026):

| Front | Today | Survey |
|---|---|---|
| Ribbon commands | **211 / 290** (72.8%) | [`PARIDADE-EXCEL.md`](docs/planejamento/PARIDADE-EXCEL.md) |
| Developer tab | **4 / 19** | same, "Desenvolvedor" section |
| Worksheet functions | **520 / 520** | [`FUNCOES-FALTANTES.md`](docs/planejamento/FUNCOES-FALTANTES.md) |
| VBA functions in the converter | **52** covered, **12** partial, **53** missing (of 117 applicable) | [`PARIDADE-VBA-FUNCOES.md`](docs/planejamento/PARIDADE-VBA-FUNCOES.md) |

The linked surveys are in Portuguese.

## What counts as 1.0

1. **Open and save** `.xlsx`, `.xlsm`, `.csv`, `.parquet` and `.fshz` without
   losing content, formatting, charts, shapes, names or validations.
2. **Every command** in the File, Home, Insert, Page Layout, Formulas, Data,
   Review and View tabs and in the contextual tabs (table, chart, shape, pivot
   table) wired to a real action, except what is listed under
   [Out of scope](#out-of-scope).
3. **Functions**: all Excel 365 functions that don't depend on an online service.
4. **Macros**: the applicable VBA functions converted, a macro recorder and
   event macros.
5. **No regressions**: engine, io and interface tests green; the user's large
   files measured on every release.

---

## Packaging

Shipped: Linux (`.deb` and portable `.tar.gz`), Windows (`.exe` installer),
macOS Apple Silicon (`.dmg`) and an update notice inside the app.

- [ ] Build Linux against an older glibc, to run on Ubuntu 22.04/24.04,
      Debian 12 and Mint (today it needs glibc 2.43, Ubuntu 26.04 or newer).
- [ ] Other formats: AppImage and `.rpm`.

## Phase 1 — Trust in daily use

What could make someone lose work or give up on day one.

- [ ] All interface tests green.
- [ ] Very large `.fshz`: saving above 4 GB and the 400 MB memory plan
      ([`PLANO-MEMORIA-400MB.md`](docs/planejamento/PLANO-MEMORIA-400MB.md)).
- [ ] Open large `.xlsx` files faster (measured with real files).
- [ ] `.xlsx` fidelity: zero-height rows, default row height per sheet, medium
      and thick borders, hidden row/column headers, zoom per sheet, table `dxf`
      formats, duplicated waterfall chart title.
- [ ] A defined name used as a range in `COUNTIF` and as a list in `MATCH`.
- [ ] Window title when opening a file that isn't `.fshz` (today "Sem título.fshz").
- [ ] Open and repair a corrupted file.
- [ ] **Several FeatherSheet windows working together** (references to another
      open workbook, `=[Sales.fshz]Sheet1!A1`, already work):
      - copy and paste between windows keeping formulas, formats and
        validations (not just the text);
      - links to a workbook while it is closed;
      - move or copy a sheet to another open workbook;
      - View → Switch Windows, Arrange All and side by side with synchronized
        scrolling;
      - a warning when the same file is opened twice (read-only the second time).

## Phase 2 — Complete the ribbon (79 commands)

In order of how much users miss them.

**Page Layout (8 / 23)** — what matters most for printing:
- [ ] Margins, Breaks, Print Titles, Print Headings, Background.
- [ ] Themes, theme Colors, Fonts and Effects.
- [ ] Arrange objects: Bring Forward, Send Backward, Align, Group, Rotate, Selection Pane.

**Insert (11 / 25):**
- [ ] Text Box, Header & Footer, Symbol, Equation.
- [ ] Sparklines, Recommended Charts.
- [ ] Timeline.
- [ ] SmartArt (the engine already exists in [`feather-shapes`](https://github.com/feather-works/feather-shapes)), Screenshot, Maps, Signature Line, Object.

**View (9 / 20):**
- [ ] Normal and Page Layout views, custom views, Ruler.
- [ ] Zoom to Selection.
- [ ] Windows: New Window, Arrange All, Split, Hide, Unhide, Switch Windows.

**Review (7 / 18):**
- [ ] Spelling and Thesaurus (local Hunspell dictionaries, as in FeatherSlide).
- [ ] Workbook Statistics, Check Accessibility, Alt Text.
- [ ] Show Comments, Convert to Comments.
- [ ] Protect Workbook, Allow Edit Ranges.
- [ ] Show Changes.

**Data (18 / 28):**
- [ ] Consolidate, Circle Invalid Data, Relationships.
- [ ] Forecast Sheet, Solver.
- [ ] Refresh All (queries and connections).
- [ ] Custom Lists, Data Form.

**Contextual tabs:**
- [ ] Table Design: Summarize with PivotTable, Insert Slicer, style options
      (header row, first and last column).
- [ ] Chart Design: Quick Layout, Styles, Switch Row/Column, Move Chart.
- [ ] Shape Format: Format Selection, Shape Fill and Outline.
- [ ] PivotTable: Sort, Clear All, Grand Totals.

**File (10 / 11):**
- [ ] Export as a web page.

## Phase 3 — Formulas

All 520 Excel functions are in ([`FUNCOES-FALTANTES.md`](docs/planejamento/FUNCOES-FALTANTES.md)
lists the known limits). What is left is calculation behavior:

- [ ] 3D references (`Jan:Dec!B5`), iterative calculation (intentional circular
      references), R1C1 style, precision as displayed, 1904 dates.
- [ ] **Rich text in a cell** (bold, color or superscript on a single word).

## Phase 4 — Macros

Macros run on [Falcão](https://github.com/feather-works/falcao), the suite's
JavaScript engine written in Rust (since v0.2.0).

- [ ] **VBA functions still missing in the converter** (53), starting with the
      most used: dates (`DateAdd`, `DateDiff`, `DatePart`, `DateSerial`,
      `DateValue`, `TimeSerial`, `TimeValue`, `Hour`, `Minute`, `Second`,
      `Weekday`, `WeekdayName`, `MonthName`, `IsDate`, `CDate`), formatting
      (`FormatNumber`, `FormatCurrency`, `FormatPercent`, `FormatDateTime`),
      text (`StrComp`, `Filter`), logic (`Choose`, `Switch`, `IsError`,
      `IsMissing`, `TypeName`, `VarType`), financial (`Pmt`, `PV`, `FV`, `NPV`,
      `IRR`, `Rate`, `NPer`, `IPmt`, `PPmt`, `MIRR`, `SLN`, `SYD`, `DDB`) and the
      rest of the [list](docs/planejamento/PARIDADE-VBA-FUNCOES.md).
- [ ] Complete the 12 partial ones.
- [ ] **Macro recorder** (record actions as JavaScript) and Use Relative References.
- [ ] **Event macros** matching Excel's (open, cell change, sheet switch…).
- [ ] Developer tab: macro security, form controls and design mode.

## Phase 5 — Charts

The engine is [`feather-charts`](https://github.com/feather-works/feather-charts).
Done: 25 types, trendlines with equation and R², error bars, max/min/average
lines, mouse tooltip, clickable legend, pivot charts.

- [ ] Sparklines and chart templates.
- [ ] Drag the title and legend inside the chart.

## Phase 6 — Advanced data

- [ ] SQLite connector (pure Rust, read-only) and `SQL()` / `QUERY()` functions.
- [ ] Timeline linked to pivot tables (slicers already work).
- [ ] Data model with relationships (Power Pivot), on top of the engine's
      column storage.
- [ ] Analyze data with a local AI (through the MCP server, on request only).
- [ ] MCP server to-dos ([`MCP-PENDENTE.md`](docs/planejamento/MCP-PENDENTE.md)).

## Phase 7 — Polish for 1.0

- [ ] Clipboard with several items.
- [ ] AutoCorrect.
- [ ] Touch mode (spaced-out ribbon).
- [ ] Right-to-left sheets.
- [ ] Inquire: compare workbooks and clean excess formatting.
- [ ] A suite-wide installer (choose all apps or just some).

---

## Out of scope

Left out because they depend on the cloud, an account or Windows, which goes
against the suite's privacy rule (everything local, no account, no telemetry):

- Real-time co-authoring, publishing to Power BI or SharePoint.
- Linked data types (Stocks, Geography), `STOCKHISTORY`, online translator and
  smart lookup, `TRANSLATE`/`DETECTLANGUAGE`.
- OLE, DDE, RTD, COM and Excel add-ins, OLAP cube functions.
- Data Streamer (serial port) and XML mapping with a schema, until someone asks.

Functions that fetch outside data (`WEBSERVICE`, `FILTERXML`) only come in as a
one-way connector, on the user's explicit request.
