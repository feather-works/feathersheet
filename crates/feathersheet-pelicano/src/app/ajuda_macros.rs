//! UI-013: guia de 128 comandos do TypeScript src/macros/guia.ts.
//! Compilado em Rust: sem JS, sem leitura de arquivo em tempo de execução.
use feather_ui::ajuda::{Topico, Linha};
use serde_json::Value;

const GUIA: &[(&str, &[(&str, &str, &str)])] = &[
    ("celulas", &[
        ("cell", "cell(address) · cells(row, col) · range(address)", "cell(\"A1\").value = 10;\ncells(2, 1).value = cell(\"A1\").value * 2;\nsheets(\"Vendas\").cell(\"C1\").value = \"Total\";"),
        ("value", "sheet.value(address)", "sheet.log(sheet.value(\"A1\"));"),
        ("formula", "sheet.formula(address)", "sheet.log(sheet.formula(\"C2\"));"),
        ("set", "sheet.set(address, value)", "sheet.set(\"B2\", \"=A2*2\");"),
        ("clear", "sheet.clear(address)", "sheet.clear(\"A1:D10\");"),
        ("style", "sheet.style(address, style)", "sheet.style(\"A1\", { bold: true, color: \"#c00000\" });"),
    ]),
    ("intervalos", &[
        ("range", "sheet.range(address)", "sheet.range(\"A1:D1\").style({ bold: true }).set(\"Total\");"),
        ("Range.values", "range.values() · range.value", "const linhas = sheet.range(\"A1:B5\").values();"),
        ("Range.formulas", "range.formulas() · range.text", "const f = sheet.range(\"C1:C5\").formulas();"),
        ("Range.set", "range.set(value | matrix)", "sheet.range(\"A1\").set([[\"Nome\", \"Total\"], [\"Ana\", 10]]);"),
        ("Range.forEach", "range.forEach((value, row, col, address) => …)", "sheet.range(\"A1:A10\").forEach((v, r, c, end) => sheet.log(end, v));"),
        ("Range.map", "range.map((value, row, col) => newValue)", "sheet.range(\"B2:B100\").map((v) => (typeof v === \"number\" ? v * 1.1 : undefined));"),
        ("Range.style", "range.style(style)", "sheet.range(\"A1:D1\").style({ bold: true, bg: \"#fff2b2\" });"),
        ("Range.clear", "range.clear()", "sheet.range(\"F1:F100\").clear();"),
        ("Range.address", "range.address", "sheet.log(sheet.selection().address);"),
        ("Range.rows", "range.rows · range.columns", "const r = sheet.range(\"A1:C4\"); sheet.log(r.rows, r.columns);"),
    ]),
    ("formatacao", &[
        ("style-keys", "{ bold, italic, underline, strike, color, bg, fontSize, fontFamily }", "sheet.style(\"A1\", { bold: true, italic: true, color: \"#1f4e79\", bg: \"#ddebf7\", fontSize: 14 });"),
        ("style-layout", "{ align, vAlign, wrap, rotation }", "sheet.style(\"A1:D1\", { align: \"center\", vAlign: \"middle\", wrap: true });"),
        ("style-number", "{ format, decimals }", "sheet.style(\"C2:C100\", { format: \"currency\", decimals: 2 });"),
    ]),
    ("selecao", &[
        ("selection", "sheet.selection()", "sheet.selection().style({ bg: \"#fff2b2\" });"),
        ("select", "sheet.select(address)", "sheet.select(\"A1\");"),
        ("lastRow", "sheet.lastRow · sheet.lastColumn", "sheet.set(\"A\" + (sheet.lastRow + 1), \"novo\");"),
        ("address", "sheet.address(row, col)", "sheet.set(sheet.address(2, 3), 10); // C2"),
    ]),
    ("abas", &[
        ("sheets", "sheet.sheets([name]) · sheet.worksheets(name)", "sheet.log(sheet.sheets());"),
        ("name", "sheet.name", "sheet.status(\"Rodando em \" + sheet.name);"),
        ("goToSheet", "sheet.goToSheet(name)", "sheet.goToSheet(\"Resumo\");"),
        ("showComponent", "sheet.showComponent(name, dados?)", "sheet.showComponent(\"cadastro\");"),
    ]),
    ("funcoes", &[
        ("defineFunction", "sheet.defineFunction(NAME, (...args) => result)", "sheet.defineFunction(\"IMPOSTO\", (v) => v * 0.15);"),
    ]),
    ("saida", &[
        ("log", "sheet.log(...values)", "sheet.log(\"A1 vale\", sheet.value(\"A1\"));"),
        ("status", "sheet.status(message)", "sheet.status(\"Pronto!\");"),
    ]),
    ("excel-valores", &[
        ("get", "sheet.get(addr)", "const total = sheet.get(\"B2\");"),
        ("setFormula", "sheet.setFormula(addr, formula)", "sheet.setFormula(\"C1:C10\", \"=A1+B1\");"),
        ("setFormulaR1C1", "sheet.setFormulaR1C1(addr, formula)", "sheet.setFormulaR1C1(\"C1:C10\", \"=R[0]C[-2]+R[0]C[-1]\");"),
        ("getText", "sheet.getText(addr)", "sheet.log(sheet.getText(\"A1\"));  // \"1.234,50\""),
        ("setNumberFormat", "sheet.setNumberFormat(addr, mask)", "sheet.setNumberFormat(\"D2:D100\", \"#,##0.00\");"),
        ("clearContents", "sheet.clearContents(addr)", "sheet.clearContents(\"A2:D100\");"),
        ("clearFormats", "sheet.clearFormats(addr)", "sheet.clearFormats(\"A1:D1\");"),
        ("copy", "sheet.copy(from, to, { paste })", "sheet.copy(\"A1:B2\", \"D1\", { paste: \"values\" });"),
        ("move", "sheet.move(from, to)", "sheet.move(\"A1:B2\", \"D1\");"),
        ("find", "sheet.find(text, { range, matchCase, wholeCell, inFormulas })", "const onde = sheet.find(\"total\", { range: \"A:A\" });"),
        ("replace", "sheet.replace(addr, old, new, { matchCase })", "const n = sheet.replace(\"A:A\", \"velho\", \"novo\");"),
        ("getAddress", "sheet.getAddress(range)", "sheet.getAddress(sheet.range(\"B2:C4\"));  // \"B2:C4\""),
        ("autoFill", "sheet.autoFill(source, destination)", "sheet.autoFill(\"A1:A2\", \"A1:A10\");"),
    ]),
    ("excel-formato", &[
        ("setFontFamily", "sheet.setFontFamily(addr, name)", "sheet.setFontFamily(\"A1:D1\", \"Inter\");"),
        ("setFontSize", "sheet.setFontSize(addr, px)", "sheet.setFontSize(\"A1\", 16);"),
        ("setBold", "sheet.setBold(addr, on = true)", "sheet.setBold(\"A1:D1\");"),
        ("setItalic", "sheet.setItalic(addr, on = true)", "sheet.setItalic(\"A1\");"),
        ("setUnderline", "sheet.setUnderline(addr, kind = \"single\")", "sheet.setUnderline(\"A1\", \"single\");"),
        ("setStrikethrough", "sheet.setStrikethrough(addr, on = true)", "sheet.setStrikethrough(\"A2\");"),
        ("setFontColor", "sheet.setFontColor(addr, color)", "sheet.setFontColor(\"A1\", \"#ef4444\");"),
        ("setBackground", "sheet.setBackground(addr, color)", "sheet.setBackground(\"A1:D1\", \"#fef08a\");"),
        ("setAlignHorizontal", "sheet.setAlignHorizontal(addr, align)", "sheet.setAlignHorizontal(\"A1:D1\", \"center\");"),
        ("setAlignVertical", "sheet.setAlignVertical(addr, align)", "sheet.setAlignVertical(\"A1\", \"middle\");"),
        ("setWrapText", "sheet.setWrapText(addr, on = true)", "sheet.setWrapText(\"A1\");"),
        ("setTextRotation", "sheet.setTextRotation(addr, degrees)", "sheet.setTextRotation(\"A1\", 45);"),
        ("setHyperlink", "sheet.setHyperlink(addr, url, text?)", "sheet.setHyperlink(\"A1\", \"https://exemplo.com\", \"Site\");"),
        ("setBorder", "sheet.setBorder(addr, options)", "sheet.setBorder(\"A1:C5\", { top: true, bottom: true, insideH: true, color: \"#cbd5e1\" });"),
        ("setOutline", "sheet.setOutline(addr, { color })", "sheet.setOutline(\"A1:C5\", { color: \"#000000\" });"),
        ("setShowGridlines", "sheet.setShowGridlines(on)", "sheet.setShowGridlines(false);"),
    ]),
    ("excel-estrutura", &[
        ("setRowHeight", "sheet.setRowHeight(row, px)", "sheet.setRowHeight(1, 28);"),
        ("setColumnWidth", "sheet.setColumnWidth(col, px)", "sheet.setColumnWidth(\"A\", 120);"),
        ("autoFitColumn", "sheet.autoFitColumn(col)", "sheet.autoFitColumn(\"A\");"),
        ("hideRow", "sheet.hideRow(row, hide = true)", "sheet.hideRow(5);"),
        ("hideColumn", "sheet.hideColumn(col, hide = true)", "sheet.hideColumn(\"B\");"),
        ("insertRow", "sheet.insertRow(row, count = 1)", "sheet.insertRow(3, 2);"),
        ("insertColumn", "sheet.insertColumn(col, count = 1)", "sheet.insertColumn(\"C\");"),
        ("deleteRow", "sheet.deleteRow(row, count = 1)", "sheet.deleteRow(7);"),
        ("deleteColumn", "sheet.deleteColumn(col, count = 1)", "sheet.deleteColumn(\"C\", 2);"),
        ("deleteCells", "sheet.deleteCells(addr, shift = \"up\")", "sheet.deleteCells(\"B2:B4\", \"up\");  // ou \"left\""),
        ("insertCells", "sheet.insertCells(addr, shift = \"down\")", "sheet.insertCells(\"B2\", \"right\");  // ou \"down\""),
        ("rowCount", "sheet.rowCount(addr)", "sheet.rowCount(\"A1:C10\");  // 10"),
        ("columnCount", "sheet.columnCount(addr)", "sheet.columnCount(\"A1:C10\");  // 3"),
    ]),
    ("excel-navegacao", &[
        ("cell", "sheet.cell(row, col) · sheet.cells(row, col)", "sheet.cell(2, 3).values();  // a C2"),
        ("getLastRow", "sheet.getLastRow(col = \"A\")", "const ultima = sheet.getLastRow(\"A\");"),
        ("getLastColumn", "sheet.getLastColumn(row = 1)", "const ultima = sheet.getLastColumn(1);"),
        ("getDataRange", "sheet.getDataRange()", "sheet.getDataRange().address;  // \"A1:F200\""),
        ("getRegion", "sheet.getRegion(addr)", "sheet.getRegion(\"A1\").values();"),
        ("offset", "sheet.offset(addr, rows, cols)", "sheet.offset(\"A1\", 1, 0);  // A2"),
        ("resize", "sheet.resize(addr, rows, cols)", "sheet.resize(\"A1\", 10, 5);  // A1:E10"),
        ("columnOf", "sheet.columnOf(addr, n)", "sheet.columnOf(\"A1:C5\", 2);  // B1:B5"),
        ("rowOf", "sheet.rowOf(addr, n)", "sheet.rowOf(\"A1:C5\", 3);  // A3:C3"),
        ("merge", "sheet.merge(addr)", "sheet.merge(\"A1:C1\");"),
        ("unmerge", "sheet.unmerge(addr)", "sheet.unmerge(\"A1:C1\");"),
        ("isMerged", "sheet.isMerged(addr)", "if (sheet.isMerged(\"A1\")) sheet.log(\"mesclada\");"),
        ("specialCells", "sheet.specialCells(addr, kind)", "const vazias = sheet.specialCells(\"A1:D20\", \"blanks\");"),
        ("activeCell", "sheet.activeCell()", "sheet.activeCell().address;"),
        ("intersect", "sheet.intersect(a, b)", "sheet.intersect(\"A1:C3\", \"B2:D5\");  // B2:C3"),
        ("union", "sheet.union(a, b)", "sheet.union(\"A1\", \"C3\");  // A1:C3"),
    ]),
    ("excel-dados", &[
        ("sort", "sheet.sort(addr, keys, { header })", "sheet.sort(\"A1:D100\", [{ col: \"B\", order: \"asc\" }], { header: true });"),
        ("removeDuplicates", "sheet.removeDuplicates(addr, cols?, { header })", "const n = sheet.removeDuplicates(\"A1:D100\", [\"A\", \"B\"], { header: true });"),
        ("setDataValidation", "sheet.setDataValidation(addr, rule)", "sheet.setDataValidation(\"A1:A9\", { type: \"list\", values: [\"Sim\", \"Não\"] });"),
        ("clearDataValidation", "sheet.clearDataValidation(addr)", "sheet.clearDataValidation(\"A1:A9\");"),
        ("addConditionalFormat", "sheet.addConditionalFormat(addr, rule)", "sheet.addConditionalFormat(\"B2:B50\", { type: \"cellValue\", operator: \"greaterThan\", value: 100, fill: \"#fecaca\" });"),
        ("addColorScale", "sheet.addColorScale(addr, { min, max, mid? })", "sheet.addColorScale(\"B2:B50\", { min: \"#ffffff\", max: \"#22c55e\" });"),
        ("addDataBar", "sheet.addDataBar(addr, { color, hideValue })", "sheet.addDataBar(\"B2:B50\", { color: \"#3b82f6\" });"),
        ("clearConditionalFormats", "sheet.clearConditionalFormats(addr)", "sheet.clearConditionalFormats(\"A1:D50\");"),
    ]),
    ("excel-pasta", &[
        ("wb.sheet", "wb.sheet(name)", "wb.sheet(\"Resumo\").set(\"A1\", 1);"),
        ("wb.addSheet", "wb.addSheet(name)", "const vendas = wb.addSheet(\"Vendas\");"),
        ("wb.deleteSheet", "wb.deleteSheet(name)", "wb.deleteSheet(\"Rascunho\");"),
        ("wb.renameSheet", "wb.renameSheet(oldName, newName)", "wb.renameSheet(\"Plan1\", \"Dados\");"),
        ("wb.copySheet", "wb.copySheet(name, newName?)", "wb.copySheet(\"Modelo\", \"Janeiro\");"),
        ("wb.moveSheet", "wb.moveSheet(name, position)", "wb.moveSheet(\"Resumo\", 1);"),
        ("wb.setTabColor", "wb.setTabColor(name, color)", "wb.setTabColor(\"Vendas\", \"#2563eb\");"),
        ("wb.setSheetVisible", "wb.setSheetVisible(name, visible)", "wb.setSheetVisible(\"Auxiliar\", false);"),
        ("wb.activate", "wb.activate(name)", "wb.activate(\"Resumo\");"),
        ("wb.freezePanes", "wb.freezePanes({ rows, cols })", "wb.freezePanes({ rows: 1, cols: 0 });"),
        ("wb.addName", "wb.addName(name, address)", "wb.addName(\"Vendas\", \"Plan1!A1:A10\");"),
        ("wb.sheetCount", "wb.sheetCount", "sheet.log(wb.sheetCount);"),
        ("wb.name", "wb.name", "sheet.log(wb.name);"),
        ("sheet.exportPdf", "sheet.exportPdf(path)", "sheet.exportPdf(\"relatorios/resumo.pdf\");"),
        ("sheet.saveAs", "sheet.saveAs(path)", "sheet.saveAs(\"copias/vendas.xlsx\");"),
        ("wb.saveSheetsAs", "wb.saveSheetsAs(sheets, path)", "wb.saveSheetsAs([\"Dados\", \"Resumo\"], \"saida/parcial.fshz\");"),
    ]),
    ("excel-app", &[
        ("app.batch", "app.batch(fn)", "app.batch(() => { sheet.set(\"A1\", 1); });"),
        ("app.calculate", "app.calculate()", "app.calculate();"),
        ("app.status", "app.status(text)", "app.status(\"Processando...\");"),
        ("app.setEventsEnabled", "app.setEventsEnabled(on)", "app.setEventsEnabled(false);"),
        ("app.rgb", "app.rgb(r, g, b)", "sheet.setBackground(\"A1\", app.rgb(37, 99, 235));"),
        ("app.vbaColorToHex", "app.vbaColorToHex(n)", "app.vbaColorToHex(255);  // \"#ff0000\""),
        ("app.isError", "app.isError(value)", "if (app.isError(app.fn.vlookup(9, tabela, 2))) sheet.log(\"não achou\");"),
        ("app.error", "app.error(code)", "sheet.set(\"A1\", app.error(\"#N/A\"));"),
    ]),
    ("excel-fn", &[
        ("app.fn.sum", "app.fn.sum(values)", "app.fn.sum(sheet.range(\"A1:A10\").values());"),
        ("app.fn.average", "app.fn.average(values)", "app.fn.average(sheet.range(\"B2:B9\").values());"),
        ("app.fn.count", "app.fn.count(values)", "app.fn.count(valores);"),
        ("app.fn.countA", "app.fn.countA(values)", "app.fn.countA(valores);"),
        ("app.fn.max", "app.fn.max(values)", "app.fn.max(valores);"),
        ("app.fn.min", "app.fn.min(values)", "app.fn.min(valores);"),
        ("app.fn.median", "app.fn.median(values)", "app.fn.median(valores);"),
        ("app.fn.countIf", "app.fn.countIf(values, criterion)", "app.fn.countIf(valores, \">100\");"),
        ("app.fn.sumIf", "app.fn.sumIf(values, criterion, sumValues?)", "app.fn.sumIf(regioes, \"Sul\", vendas);"),
        ("app.fn.averageIf", "app.fn.averageIf(values, criterion, avgValues?)", "app.fn.averageIf(notas, \">=7\");"),
        ("app.fn.vlookup", "app.fn.vlookup(key, table, col, exact = true)", "app.fn.vlookup(\"B\", sheet.range(\"A1:C9\").values(), 3);"),
        ("app.fn.hlookup", "app.fn.hlookup(key, table, row, exact = true)", "app.fn.hlookup(\"Mar\", tabela, 2);"),
        ("app.fn.match", "app.fn.match(value, list, type = 0)", "app.fn.match(\"b\", [\"a\", \"b\", \"c\"]);  // 2"),
        ("app.fn.index", "app.fn.index(matrix, row, col?)", "app.fn.index(tabela, 2, 3);"),
    ]),
];

pub fn montar(root: &Value) -> Vec<Topico> {
    GUIA.iter().map(|(id, entries)| {
        let titulo = root["macros"]["secoes"][*id]
            .as_str().unwrap_or(id).to_owned();
        let mut linhas = Vec::new();
        for (nome, assinatura, exemplo) in *entries {
            linhas.push(Linha::Exemplo((*assinatura).to_owned()));
            linhas.push(Linha::Texto(
                root["macros"]["funcoes"][*nome]
                    .as_str().unwrap_or(nome).to_owned()
            ));
            linhas.push(Linha::Exemplo((*exemplo).to_owned()));
        }
        Topico { id:(*id).to_owned(), titulo, linhas }
    }).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn todas_as_funcoes_do_guia_ts_foram_portadas() {
        assert_eq!(GUIA.len(),15);
        assert_eq!(GUIA.iter().map(|(_,e)|e.len()).sum::<usize>(),134);
        let j:Value=serde_json::from_str(include_str!("../../../../src/idiomas/pt-BR.json")).unwrap();
        for (_,entries) in GUIA {
            for (nome,_,_) in *entries {
                assert!(j["macros"]["funcoes"][*nome].is_string(),"falta descrição: {nome}");
            }
        }
        assert_eq!(montar(&j).len(),15);
    }
}
