//! Interpretador Rust para as macros JavaScript do documento.
pub mod enderecos;
mod operacoes_valores;
mod funcoes_criterio;
mod funcoes_busca;
mod copiar;
mod movimento;
mod preenchimento;
mod operacoes_formato;
mod bordas;
mod operacoes_dados;
mod validacoes;
mod condicionais;
mod navegacao;
mod navegacao_intervalos;
mod estrutura;
mod mesclagem;
mod pasta;
pub use pasta::{Contexto, PedidoArquivo};
use feather_macros::HospedeiroMacros;
use feathersheet_core::{
    celulas::{refs::parse_range_ref, estilo_parcial::{Campo, EstiloParcial, mesclar, nome_do_formato}},
    planilha::{workbook::Workbook, sheet::edicao::Snap},
    Value, MAX_COLS, MAX_ROWS,
};
use serde_json::{json, Value as Json};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

#[derive(Default, Debug)]
pub struct Resultado {
    pub alterou: bool,
    pub saida: Vec<String>,
    pub erro: Option<String>,
    /// Última mensagem solicitada pela macro, só aplicável se a execução terminar bem.
    pub status: Option<String>,
    /// Pergunta Sim/Não ainda sem resposta. A execução foi desfeita: o
    /// aplicativo pergunta à pessoa e roda de novo com a resposta em
    /// [`Contexto::respostas`].
    pub pergunta: Option<String>,
    /// Mensagens de `alert`/`MsgBox`, na ordem, para o aplicativo mostrar.
    pub mensagens: Vec<String>,
    /// Aba que a macro deixou ativa (`wb.activate`, `sheet.activate()`).
    pub aba_ativa: Option<usize>,
    /// Arquivos pedidos (`sheet.exportPdf`, `wb.saveSheetsAs`), na ordem. A
    /// macro não grava nada: o aplicativo grava estes se ela terminou bem.
    pub arquivos: Vec<PedidoArquivo>,
}

struct Estado {
    pasta: Workbook,
    alteradas: BTreeSet<usize>,
    status: Option<String>,
    aba_ativa: usize,
    extra: pasta::Acumulado,
}

/// Adaptador específico do Sheet para o motor de macros compartilhado.
/// Somente o produto conhece o Workbook, endereços A1 e suas transações.
struct HospedeiroSheet {
    estado: Rc<RefCell<Estado>>,
}

fn valor_json(valor: Value) -> Json {
    match valor {
        Value::Empty => Json::Null,
        Value::Number(n) => json!(n),
        Value::Text(s) => json!(s),
        Value::Bool(b) => json!(b),
        // Diferencia erro de planilha de texto literal "#N/A" ao
        // atravessar a ponte JSON para JavaScript.
        Value::Error(e) => json!({"__ftCellError":e.code.as_str(),"detail":e.detail}),
    }
}

fn bruto(v: &Json) -> String {
    match v {
        Json::Null => String::new(),
        Json::String(s) => s.clone(),
        Json::Bool(b) => if *b { "VERDADEIRO" } else { "FALSO" }.into(),
        outro => outro.to_string(),
    }
}

impl HospedeiroMacros for HospedeiroSheet {
    fn chamar(&self, op: &str, dados: &Json) -> Result<Json, String> {
        let mut e = self.estado.borrow_mut();
        if pasta::OPERACOES.contains(&op) {
            let nomes = e.pasta.nomes();
            let ids = e.pasta.ids();
            let ativa = e.aba_ativa;
            return pasta::executar(op, dados, &mut e.extra, &nomes, &ids, ativa)
                .unwrap_or_else(|| Err(format!("Operação desconhecida: {op}")));
        }
        let resposta = if op == "nomes" {
            json!(e.pasta.nomes())
        } else if op == "indiceAba" {
            // API wb.sheet de api-pasta.ts: nomes sem distinção
            // de maiúsculas/minúsculas, usando Workbook::indice_por_nome.
            let nome = dados[0].as_str().ok_or("O nome da aba precisa ser texto")?;
            json!(e.pasta.indice_por_nome(nome.trim()))
        } else if matches!(op, "worksheetCountIf" | "worksheetSumIf"
            | "worksheetAverageIf" | "worksheetAverage" | "worksheetMedian"
            | "worksheetSum" | "worksheetCount" | "worksheetCountA"
            | "worksheetMax" | "worksheetMin") {
            funcoes_criterio::executar(op, dados)?
        } else if matches!(op, "worksheetVLookup" | "worksheetHLookup"
            | "worksheetMatch" | "worksheetIndex") {
            funcoes_busca::executar(op, dados)?
        } else if op == "appCalculate" {
            let aba = e.aba_ativa;
            let folha = e.pasta.aba_mut(aba)
                .ok_or("app.calculate: aba ativa inexistente")?;
            folha.sheet.invalidar_todos_calculos();
            Json::Null
        } else if op == "appStatus" {
            let mensagem = dados.get(0).and_then(Json::as_str)
                .ok_or("app.status: informe uma mensagem de texto")?;
            // Não permite textos enormes na barra de status.
            e.status = Some(mensagem.chars().take(1024).collect());
            Json::Null
        } else if op == "appError" {
            // Os códigos de erro pertencem ao motor Rust, não a JS.
            let codigo = dados.get(0).and_then(Json::as_str)
                .and_then(feathersheet_core::celulas::values::ErrorCode::from_code)
                .ok_or("app.error: código de erro de planilha inválido")?;
            let detalhe = dados.get(1).and_then(Json::as_str)
                .map(|s|s.chars().take(2048).collect::<String>());
            json!({"__ftCellError":codigo.as_str(),"detail":detalhe})
        } else if op == "appRgb" {
            let canal = |i: usize| -> Result<u8, String> {
                let n = dados[i].as_u64().ok_or("RGB exige inteiros de 0 a 255")?;
                u8::try_from(n).map_err(|_|"RGB exige canais de 0 a 255".into())
            };
            json!(enderecos::rgb_vba(canal(0)?, canal(1)?, canal(2)?))
        } else if op == "appVbaColorToHex" {
            let n = dados[0].as_i64().ok_or("Cor VBA exige numero inteiro")?;
            json!(enderecos::vba_cor_hex(n as u32))
        } else {
            let aba = dados[0].as_u64().ok_or_else(|| "Índice de aba inválido".to_string())? as usize;
            if aba >= e.pasta.len() { return Err("Aba inexistente".to_string()); }
            // API de navegação: cada comando é escalar; nunca gera
            // uma matriz 10M x 16K para devolver metadados.
            if matches!(op, "lastRow" | "lastColumn" | "getLastRow"
                | "getLastColumn" | "getDataRange" | "address")
            {
                use feathersheet_core::celulas::refs::{cell_name,col_index};
                let s=&mut e.pasta.aba_mut(aba).unwrap().sheet;
                return match op {
                    "lastRow" => Ok(json!(s.max_row_ts() + 1)),
                    "lastColumn" => Ok(json!(s.max_col_ts() + 1)),
                    "getLastRow" => {
                        let coluna=if let Some(nome)=dados[1].as_str() {
                            col_index(nome.trim()).ok_or_else(||format!(
                                "Coluna inválida: {nome} (use A a XFD)"
                            ))?
                        } else {
                            let n=dados[1].as_u64()
                                .ok_or("Coluna deve ser letra ou número inteiro a partir de 1")?;
                            if n==0 || n>u64::from(MAX_COLS) {
                                return Err("Coluna fora dos limites (1 a 16384)".into());
                            }
                            (n-1) as u32
                        };
                        Ok(json!(s.ultima_linha_na_coluna(coluna)))
                    }
                    "getLastColumn" => {
                        let n=dados[1].as_u64().ok_or(
                            "Linha deve ser número inteiro a partir de 1"
                        )?;
                        if n==0 || n>u64::from(MAX_ROWS) {
                            return Err(format!("Linha fora dos limites (1 a {MAX_ROWS})"));
                        }
                        Ok(json!(s.ultima_coluna_na_linha((n-1) as u32)))
                    }
                    "getDataRange" => {
                        s.acertar_limites();
                        if s.is_empty() { Ok(Json::Null) }
                        else {
                            let usada=s.used_rect();
                            let ret=feathersheet_core::celulas::refs::Rect::new(
                                0,0,usada.r1,usada.c1,
                            );
                            Ok(json!(operacoes_valores::endereco_a1(&ret)))
                        }
                    }
                    "address" => {
                        let linha=dados[1].as_u64()
                            .ok_or("Linha deve ser inteiro a partir de 1")?;
                        let coluna=dados[2].as_u64()
                            .ok_or("Coluna deve ser inteiro a partir de 1")?;
                        if linha==0 || linha>u64::from(MAX_ROWS)
                            || coluna==0 || coluna>u64::from(MAX_COLS) {
                            return Err("Endereço fora dos limites da planilha".into());
                        }
                        Ok(json!(cell_name((linha-1) as u32,(coluna-1) as u32)))
                    }
                    _ => unreachable!(),
                };
            }
            if matches!(op, "insertRow" | "deleteRow" | "insertColumn" | "deleteColumn") {
                let estado: &mut Estado = &mut *e;
                movimento::linhas_colunas(&mut estado.pasta, &mut estado.alteradas, aba, op, &dados[1], &dados[2])?;
                return Ok(Json::Null);
            }
            if matches!(op, "setRowHeight" | "setColumnWidth" | "hideRow" | "hideColumn") {
                // API de dimensões: o armazenamento e o undo já existem
                // em Sheet::definir_dimensao / alterar_visibilidade_dimensoes.
                let acao = estrutura::preparar(op, dados)?;
                let iniciou = e.alteradas.insert(aba);
                if iniciou {
                    e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                }
                let mut resultado = Ok(false);
                e.pasta.editar(aba, |sheet| {
                    resultado = estrutura::aplicar(sheet, acao);
                });
                let mudou = resultado?;
                if iniciou && !mudou {
                    e.pasta.aba_mut(aba).unwrap().sheet.reverter_transacao();
                    e.alteradas.remove(&aba);
                }
                return Ok(Json::Null);
            }
            if matches!(op, "offset" | "resize" | "columnOf" | "rowOf") {
                return navegacao_intervalos::executar(op, dados);
            }
            // getRegion do api-navegacao.ts: o algoritmo já é nativo
            // em feathersheet-core::operacoes::ops::regiao_atual. Não
            // duplicar a expansão de vizinhança no host das macros.
            if op == "getRegion" {
                let endereco=dados[1].as_str()
                    .ok_or("getRegion: informe o endereço A1 da seleção")?;
                let area=parse_range_ref(endereco)
                    .ok_or_else(||format!("getRegion: endereço inválido: {endereco}"))?
                    .normalized();
                if area.sheet.is_some() {
                    return Err("Use wb.sheet(nome).getRegion(endereço) para outra aba".into());
                }
                let folha=&e.pasta.aba(aba).unwrap().sheet;
                let regiao=feathersheet_core::operacoes::ops::regiao_atual(folha,area);
                return Ok(json!(operacoes_valores::endereco_a1(&regiao)));
            }
            if op == "find" {
                let texto = dados[1].as_str().ok_or("Texto da busca inválido")?;
                let opcoes = &dados[2];
                let sheet = &e.pasta.aba(aba).unwrap().sheet;
                if sheet.is_empty() { return Ok(Json::Null); }
                let usada = sheet.used_rect();
                let pedido = if let Some(endereco) = opcoes["range"].as_str() {
                    let pedido = parse_range_ref(endereco)
                        .ok_or_else(||format!("Intervalo de busca inválido: {endereco}"))?
                        .normalized();
                    if pedido.sheet.is_some() {
                        return Err("Use wb.sheet(nome).find(...) para outra aba".into());
                    }
                    pedido
                } else {
                    usada.clone()
                };
                let area = (
                    pedido.r0.max(usada.r0), pedido.c0.max(usada.c0),
                    pedido.r1.min(usada.r1), pedido.c1.min(usada.c1),
                );
                if area.0 > area.2 || area.1 > area.3 {
                    return Ok(Json::Null);
                }
                let area = feathersheet_core::celulas::refs::Rect::new(
                    area.0,area.1,area.2,area.3,
                );
                if area.area() > 500_000 {
                    return Err("A busca excede 500.000 células; indique options.range".into());
                }
                return Ok(json!(operacoes_valores::localizar(
                    &e.pasta,aba,&area,texto,
                    opcoes["matchCase"].as_bool().unwrap_or(false),
                    opcoes["wholeCell"].as_bool().unwrap_or(false),
                    opcoes["inFormulas"].as_bool().unwrap_or(false),
                )));
            }
            let endereco = dados[1].as_str().ok_or_else(|| "Endereço precisa ser texto A1".to_string())?;
            let area = parse_range_ref(endereco).ok_or_else(|| format!("Endereço inválido: {endereco}"))?.normalized();
            if area.sheet.is_some() { return Err("Use wb.sheet(nome).range(endereço) para outra aba".to_string()); }
            // `Cells.Copy` / `Columns("A:C").Copy` / `Columns("A").TextToColumns`:
            // como o Excel, só o que tem dado (a área usada da aba).
            let area = if matches!(op, "copy" | "textToColumns") && area.area() > 500_000 {
                let usada = e.pasta.aba(aba).map(|a| a.sheet.used_rect()).ok_or("Aba inexistente")?;
                let (r0, c0, r1, c1) = (area.r0.max(usada.r0), area.c0.max(usada.c0), area.r1.min(usada.r1), area.c1.min(usada.c1));
                if r0 > r1 || c0 > c1 { return Ok(Json::Null); }
                // A colagem ancora no canto pedido: o recorte começa nele.
                feathersheet_core::celulas::refs::Rect::new(area.r0, area.c0, r1, c1)
            } else { area };
            // Leitura de uma única célula/metadata: sem percorrer A:A.
            if !matches!(op, "area" | "getText" | "getAddress" | "intersect" | "union" | "rowCount" | "columnCount" | "isMerged" | "unmerge" | "setDataValidation" | "clearDataValidation"
                | "addConditionalFormat" | "addColorScale" | "addDataBar" | "clearConditionalFormats")
                && u64::from(area.r1-area.r0+1) * u64::from(area.c1-area.c0+1) > 500_000 {
                return Err("Restrinja o intervalo a 500.000 células".to_string());
            }
            match op {
                "addConditionalFormat" | "addColorScale" | "addDataBar" | "clearConditionalFormats" => {
                    let novo=condicionais::preparar(
                        &e.pasta.aba(aba).unwrap().sheet,&area,op,&dados[2],
                    )?;
                    if e.pasta.aba(aba).unwrap().sheet.r != novo {
                        if e.alteradas.insert(aba) {
                            e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                        }
                        e.pasta.editar(aba,|sheet|{sheet.alterar_condicionais(novo,&area);});
                    }
                    Json::Null
                }
                "setDataValidation" | "clearDataValidation" => {
                    // Metadados de validação: não percorremos a grade.
                    // Planeja e serializa antes de abrir a transação.
                    let novo = validacoes::preparar(
                        &e.pasta.aba(aba).unwrap().sheet,&area,op,&dados[2],
                    )?;
                    let precisa = e.pasta.aba(aba).unwrap().sheet.va != novo;
                    if precisa {
                        if e.alteradas.insert(aba) {
                            e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                        }
                        e.pasta.editar(aba,|sheet| {
                            sheet.alterar_validacoes(novo,&area);
                        });
                    }
                    Json::Null
                }
                "isMerged" => {
                    json!(mesclagem::esta_mesclada(
                        &e.pasta.aba(aba).unwrap().sheet, &area,
                    ))
                }
                "merge" | "unmerge" => {
                    let plano=mesclagem::preparar(
                        &e.pasta.aba(aba).unwrap().sheet,&area,op=="merge",
                    )?;
                    if plano.tem_alteracoes() {
                        if e.alteradas.insert(aba) {
                            e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                        }
                        e.pasta.editar(aba,|sheet|mesclagem::aplicar(sheet,&area,plano));
                    }
                    Json::Null
                }
                "specialCells" => {
                    let tipo = dados[2].as_str()
                        .ok_or("specialCells exige o tipo: blanks, formulas, constants ou errors")?;
                    json!(navegacao::celulas_especiais(&e.pasta, aba, &area, tipo)?)
                }
                "intersect" | "union" => {
                    // Reaproveita as operações de retângulo já portadas
                    // de enderecos.ts; não cria um motor de geometria novo.
                    let segundo = dados[2].as_str()
                        .ok_or("O segundo intervalo precisa ser um endereço A1")?;
                    let outro = parse_range_ref(segundo)
                        .ok_or_else(||format!("Intervalo inválido: {segundo}"))?
                        .normalized();
                    if outro.sheet.is_some() {
                        return Err("Use a mesma aba para intersect e union".into());
                    }
                    if op == "intersect" {
                        json!(enderecos::intersecao(&area, &outro)
                            .map(|r| operacoes_valores::endereco_a1(&r)))
                    } else {
                        json!(operacoes_valores::endereco_a1(
                            &enderecos::uniao(&area, &outro)))
                    }
                }
                "setBorder" | "setOutline" => {
                    let opts = bordas::preparar(&area, op, &dados[2])?;
                    let iniciou = e.alteradas.insert(aba);
                    if iniciou {
                        e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                    }
                    let mut mudadas = 0;
                    e.pasta.editar(aba, |sheet| {
                        mudadas = bordas::aplicar(sheet, &area, &opts);
                    });
                    if iniciou && mudadas == 0 {
                        e.pasta.aba_mut(aba).unwrap().sheet.reverter_transacao();
                        e.alteradas.remove(&aba);
                    }
                    Json::Null
                }
                comando if operacoes_formato::comando(comando) => {
                    let patch = operacoes_formato::preparar(op, dados)?;
                    let texto = if op == "setHyperlink" {
                        dados[3].as_str()
                    } else { None };
                    if op == "setHyperlink" && !dados[3].is_null() && texto.is_none() {
                        return Err("O texto do hyperlink deve ser uma string".into());
                    }
                    let iniciou = e.alteradas.insert(aba);
                    if iniciou {
                        e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                    }
                    let mut mudadas = 0;
                    e.pasta.editar(aba, |sheet| {
                        mudadas = operacoes_formato::aplicar(sheet, &area, &patch, texto);
                    });
                    if iniciou && mudadas == 0 {
                        e.pasta.aba_mut(aba).unwrap().sheet.reverter_transacao();
                        e.alteradas.remove(&aba);
                    }
                    Json::Null
                }
                "autoFill" => {
                    let alvo = dados[2].as_str()
                        .ok_or("O destino de autoFill precisa ser texto A1")?;
                    let alvo = parse_range_ref(alvo)
                        .ok_or_else(||format!("Destino inválido: {alvo}"))?
                        .normalized();
                    if !preenchimento::validar(&area, &alvo)? {
                        json!(0)
                    } else {
                        // A série, a fórmula deslocada e os estilos são
                        // calculados pelo motor Rust existente.
                        let iniciou = e.alteradas.insert(aba);
                        if iniciou {
                            e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                        }
                        let mut preenchidas=0;
                        e.pasta.editar(aba, |sheet| {
                            preenchidas = preenchimento::aplicar(sheet, &area, &alvo);
                        });
                        if iniciou && preenchidas == 0 {
                            e.pasta.aba_mut(aba).unwrap().sheet.reverter_transacao();
                            e.alteradas.remove(&aba);
                        }
                        json!(preenchidas)
                    }
                }
                "deleteCells" | "insertCells" => {
                    let padrao = if op == "deleteCells" { "up" } else { "down" };
                    let sentido = dados[2].as_str().unwrap_or(padrao);
                    let estado: &mut Estado = &mut *e;
                    movimento::deslocar_celulas(
                        &mut estado.pasta, &mut estado.alteradas,
                        aba, &area, op == "insertCells", sentido,
                    )?;
                    Json::Null
                }
                "move" => {
                    let destino = dados[2].as_str()
                        .ok_or("Destino de move precisa ser endereço A1")?;
                    let destino = parse_range_ref(destino)
                        .ok_or_else(||format!("Destino inválido: {destino}"))?
                        .normalized();
                    let estado: &mut Estado = &mut *e;
                    movimento::mover(
                        &mut estado.pasta, &mut estado.alteradas,
                        aba, &area, &destino,
                    )?;
                    Json::Null
                }
                "copy" => {
                    // 1. Planejar e congelar a origem ANTES de alterar a
                    // planilha: cópias sobrepostas têm a mesma semântica TS.
                    let destino = dados[2].as_str()
                        .ok_or("Destino da cópia precisa ser endereço A1")?;
                    // Colar em outra aba: { sheet: "Nome" } ou "Nome!B2" / "'Nome X'!B2".
                    let (prefixo, endereco) = match destino.rsplit_once('!') {
                        Some((a, b)) => (Some(a.trim_matches('\'').replace("''", "'")), b),
                        None => (None, destino),
                    };
                    let destino = parse_range_ref(endereco)
                        .ok_or_else(||format!("Destino inválido: {destino}"))?
                        .normalized();
                    let nome_destino = dados[3]["sheet"].as_str().map(str::to_owned).or(prefixo);
                    let alvo = match nome_destino {
                        Some(n) => (0..e.pasta.len()).find(|&i| e.pasta.aba(i)
                            .is_some_and(|a| a.sheet.name.eq_ignore_ascii_case(&n)))
                            .ok_or_else(|| format!("Aba de destino inexistente: {n}"))?,
                        None => aba,
                    };
                    let modo = copiar::Modo::ler(
                        dados[3]["paste"].as_str().unwrap_or("all")
                    )?;
                    let plano = copiar::preparar(
                        &e.pasta, aba, &area, &destino, modo,
                    )?;
                    if let Some(plano) = plano {
                        let iniciou = e.alteradas.insert(alvo);
                        if iniciou {
                            e.pasta.aba_mut(alvo).unwrap().sheet.abrir_transacao();
                        }
                        let mut alteradas = 0;
                        e.pasta.editar(alvo, |sheet| {
                            alteradas = copiar::aplicar(sheet, plano);
                        });
                        if iniciou && alteradas == 0 {
                            e.pasta.aba_mut(alvo).unwrap().sheet.reverter_transacao();
                            e.alteradas.remove(&alvo);
                        }
                    }
                    Json::Null
                }
                "clearContents" | "clearFormats" | "replace" => {
                    let antigo = if op == "replace" {
                        Some(dados[2].as_str().ok_or("Texto de busca inválido")?)
                    } else { None };
                    let novo = if op == "replace" {
                        dados[3].as_str().ok_or("Texto de substituição inválido")?
                    } else { "" };
                    let match_case = dados[4]["matchCase"].as_bool().unwrap_or(false);
                    if antigo == Some("") {
                        return Ok(json!(0));
                    }
                    let transacao_nova = e.alteradas.insert(aba);
                    if transacao_nova {
                        e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                    }
                    let mut modificadas = 0;
                    e.pasta.editar(aba, |sheet| {
                        modificadas = match op {
                            "clearContents" => operacoes_valores::limpar_conteudo(sheet, &area),
                            "clearFormats" => operacoes_valores::limpar_formatos(sheet, &area),
                            _ => operacoes_valores::substituir(
                                sheet, &area, antigo.unwrap_or_default(), novo, match_case,
                            ),
                        };
                    });
                    if modificadas == 0 && transacao_nova {
                        // Sem alteração real, não deixa transação vazia
                        // nem marca o documento como modificado.
                        e.pasta.aba_mut(aba).unwrap().sheet.reverter_transacao();
                        e.alteradas.remove(&aba);
                    }
                    if op == "replace" { json!(modificadas) } else { Json::Null }
                }
                "sort" | "removeDuplicates" | "textToColumns" => {
                    // A validação e o planejamento não alteram nada. Só
                    // abrimos uma transação quando há mudança de verdade.
                    let plano = operacoes_dados::preparar(&e.pasta, aba, &area, op, dados)?;
                    if let Some(mudanca) = plano.mudanca {
                        if e.alteradas.insert(aba) {
                            e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                        }
                        e.pasta.editar(aba, |sheet| {
                            operacoes_dados::aplicar(sheet, &area, mudanca);
                        });
                    }
                    plano.resposta
                }
                "setNumberFormat" => {
                    let mascara = dados[2].as_str()
                        .ok_or("A máscara de formato deve ser texto")?;
                    let formato = enderecos::mascara_para_estilo(mascara)?;
                    let mut patch = EstiloParcial::new();
                    patch.insert("format".into(),match nome_do_formato(formato.formato) {
                        Some(nome) => Campo::Texto(nome.into()),
                        None => Campo::Nulo,
                    });
                    patch.insert("decimals".into(),match formato.casas {
                        Some(casas) => Campo::Numero(f64::from(casas)),
                        None => Campo::Nulo,
                    });
                    patch.insert("numFmt".into(),match formato.codigo {
                        Some(codigo) => Campo::Texto(codigo),
                        None => Campo::Nulo,
                    });
                    if e.alteradas.insert(aba) {
                        e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao();
                    }
                    e.pasta.editar(aba, |sheet| {
                        for linha in area.r0..=area.r1 {
                            for coluna in area.c0..=area.c1 {
                                // set_style() abre e fecha a própria transação;
                                // aplicar snapshots mantém o UNDO da macro
                                // inteira como um só passo, como em set().
                                let raw = sheet.get_raw(linha,coluna);
                                let estilo = mesclar(
                                    sheet.estilo_mesclado(linha,coluna).as_ref(),
                                    &patch,
                                );
                                sheet.apply(linha,coluna,Some(Snap {
                                    raw, style: Some(estilo),
                                }),true);
                            }
                        }
                    });
                    Json::Null
                }
                "area" => json!([area.r0,area.c0,area.r1,area.c1]),
                "rowCount" => json!(area.r1 - area.r0 + 1),
                "columnCount" => json!(area.c1 - area.c0 + 1),
                "getText" => json!(operacoes_valores::texto_exibido(&e.pasta,aba,area.r0,area.c0)),
                "getAddress" => json!(operacoes_valores::endereco_a1(&area)),
                "get" => valor_json(e.pasta.valor(aba,area.r0,area.c0)),
                "raw" => json!(e.pasta.aba(aba).unwrap().sheet.get_raw(area.r0,area.c0)),
                "values" | "formulas" => json!((area.r0..=area.r1).map(|r| {
                    (area.c0..=area.c1).map(|c| if op == "values" { valor_json(e.pasta.valor(aba,r,c)) }
                        else { json!(e.pasta.aba(aba).unwrap().sheet.get_raw(r,c)) }).collect::<Vec<_>>()
                }).collect::<Vec<_>>()),
                "set" | "clear" | "setFormula" | "setFormulaR1C1" => {
                    let mut escritas = Vec::new();
                    if let Some(linhas) = dados[2].as_array().filter(|_| op == "set") {
                        for (dr,linha) in linhas.iter().enumerate() {
                            let campos = linha.as_array().cloned().unwrap_or_else(|| vec![linha.clone()]);
                            for (dc,valor) in campos.iter().enumerate() {
                                let r = area.r0 as usize + dr;
                                let c = area.c0 as usize + dc;
                                if r >= MAX_ROWS as usize || c >= MAX_COLS as usize || escritas.len() >= 500_000 {
                                    return Err("A matriz excede os limites de escrita".to_string());
                                }
                                escritas.push((r as u32,c as u32,bruto(valor)));
                            }
                        }
                    } else {
                        let texto = if op == "clear" {
                            String::new()
                        } else { bruto(&dados[2]) };
                        if (op == "setFormula" || op == "setFormulaR1C1")
                            && !texto.trim_start().starts_with('=') {
                            return Err("A fórmula deve começar com '='".into());
                        }
                        for r in area.r0..=area.r1 {
                            for c in area.c0..=area.c1 {
                                let convertido = match op {
                                    "setFormula" => enderecos::deslocar_formula(
                                        &texto,
                                        i64::from(r) - i64::from(area.r0),
                                        i64::from(c) - i64::from(area.c0),
                                    ),
                                    "setFormulaR1C1" => enderecos::r1c1_para_a1(
                                        &texto, r + 1, c + 1,
                                    ),
                                    _ => texto.clone(),
                                };
                                escritas.push((r,c,convertido));
                            }
                        }
                    }
                    if !escritas.is_empty() {
                        if e.alteradas.insert(aba) { e.pasta.aba_mut(aba).unwrap().sheet.abrir_transacao(); }
                        e.pasta.editar(aba, |sheet| {
                            for (r,c,texto) in escritas { sheet.set_cell(r,c,&texto); }
                        });
                    }
                    Json::Null
                }
                _ => return Err(format!("Operação de macro ainda não migrada: {op}")),
            }
        };
        Ok(resposta)
    }
}

/// Executa só por comando explícito. Nenhum filesystem, rede ou DOM é exposto.
/// Escritas são agrupadas por aba e revertidas se a execução lançar erro.
pub fn executar(pasta: &mut Workbook, aba: usize, codigo: &str) -> Resultado {
    executar_com(pasta, aba, codigo, Contexto::default())
}

/// Como [`executar`], com o arquivo, a seleção e as respostas já dadas.
pub fn executar_com(pasta: &mut Workbook, aba: usize, codigo: &str, contexto: Contexto) -> Resultado {
    if aba >= pasta.len() {
        return Resultado { erro: Some("Aba inexistente".into()), ..Default::default() };
    }
    // O compartilhado apenas executa JS. O Sheet é dono do Workbook e do undo.
    let estado = Rc::new(RefCell::new(Estado {
        pasta: std::mem::take(pasta), alteradas: BTreeSet::new(), status: None,
        aba_ativa: aba,
        extra: pasta::Acumulado { contexto, ..Default::default() },
    }));
    let host: Rc<dyn HospedeiroMacros> = Rc::new(HospedeiroSheet {
        estado: Rc::clone(&estado),
    });
    // O prelúdio fica na MESMA linha da 1ª linha do código: as linhas do erro
    // batem com as do editor; só a coluna da linha 1 anda (ver `posicoes`).
    let prefixo = format!("(function() {{ const sheet=__sheet({aba}), planilha=sheet; const range=sheet.range, cell=sheet.cell, cells=sheet.cell, worksheets=wb.sheet, sheets=wb.sheet; const valores={{}}; __sel[{aba}]=__ft('selecaoInicial',[]); ");
    let script = format!("{prefixo}{codigo}\n}})()");
    let mut resultado = feather_macros::executar_com_origem(host, API, &script, Some(posicoes::ORIGEM));
    resultado.erro = resultado.erro.map(|e| posicoes::ajustar_erro(&e, prefixo.chars().count(), codigo));
    let mut e = estado.borrow_mut();
    let extra = std::mem::take(&mut e.extra);
    // Pergunta sem resposta: a macro seguiu com "Não", então nada fica.
    let vale = resultado.erro.is_none() && extra.pergunta.is_none();
    let alteradas: Vec<_> = e.alteradas.iter().copied().collect();
    for aba in &alteradas {
        e.pasta.editar(*aba, |sheet| {
            if vale { sheet.fechar_transacao(); } else { sheet.reverter_transacao(); }
        });
    }
    if vale {
        for (nome, area) in &extra.nomes { e.pasta.definir_nome(nome, area.clone()); }
    }
    *pasta = std::mem::take(&mut e.pasta);
    Resultado {
        alterou: vale && (!alteradas.is_empty() || !extra.nomes.is_empty()),
        saida: resultado.saida,
        status: if vale {e.status.take()} else {None},
        // A pergunta vem antes do erro: ele pode ser só o caminho do "Não".
        erro: if extra.pergunta.is_none() { resultado.erro } else { None },
        mensagens: if extra.pergunta.is_none() { extra.mensagens } else { Vec::new() },
        aba_ativa: if vale { extra.ativou } else { None },
        arquivos: if vale { extra.arquivos } else { Vec::new() },
        pergunta: extra.pergunta,
    }
}

const API: &str = r#"
// O Boa vem sem `intl` (seriam MB de dados ICU): toLocaleString ignorava a
// língua e devolvia "Mon Oct 19 2026…". Aqui, pt-BR (padrão), en-US e es-ES.
// shortcut: só essas três línguas e moeda BRL/USD/EUR; outra língua cai no pt-BR.
(() => {
    const P = n => String(n).padStart(2, '0');
    const lingua = l => { l = String(l || 'pt-BR').toLowerCase(); return l.startsWith('en') ? 'en' : l.startsWith('es') ? 'es' : 'pt'; };
    const data = (x, l) => {
        const [d, m, a] = [x.getDate(), x.getMonth() + 1, x.getFullYear()];
        return l === 'en' ? m + '/' + d + '/' + a : l === 'es' ? d + '/' + m + '/' + a : P(d) + '/' + P(m) + '/' + a;
    };
    const hora = (x, l) => {
        if (l !== 'en') return P(x.getHours()) + ':' + P(x.getMinutes()) + ':' + P(x.getSeconds());
        const h = x.getHours() % 12 || 12;
        return h + ':' + P(x.getMinutes()) + ':' + P(x.getSeconds()) + (x.getHours() < 12 ? ' AM' : ' PM');
    };
    Date.prototype.toLocaleDateString = function (l) { return data(this, lingua(l)); };
    Date.prototype.toLocaleTimeString = function (l) { return hora(this, lingua(l)); };
    Date.prototype.toLocaleString = function (l) { l = lingua(l); return data(this, l) + ', ' + hora(this, l); };
    const numeroOriginal = Number.prototype.toLocaleString;
    Number.prototype.toLocaleString = function (l, o) {
        o = o || {}; l = lingua(l);
        let n = Number(this);
        if (!isFinite(n)) return numeroOriginal.call(n);
        const moeda = o.style === 'currency', pct = o.style === 'percent';
        if (pct) n *= 100;
        const min = o.minimumFractionDigits ?? (moeda ? 2 : 0);
        const max = Math.max(min, o.maximumFractionDigits ?? (moeda ? 2 : pct ? 0 : 3));
        let [int, dec = ''] = Math.abs(n).toFixed(max).split('.');
        while (dec.length > min && dec.endsWith('0')) dec = dec.slice(0, -1);
        const [mil, virg] = l === 'en' ? [',', '.'] : ['.', ','];
        if (o.useGrouping !== false) int = int.replace(/\B(?=(\d{3})+(?!\d))/g, mil);
        let s = (n < 0 ? '-' : '') + int + (dec ? virg + dec : '');
        if (pct) s += l === 'en' ? '%' : ' %';
        if (moeda) {
            const simbolo = { BRL: 'R$', USD: '$', EUR: '€' }[String(o.currency || 'BRL').toUpperCase()] || o.currency;
            s = l === 'es' ? s + ' ' + simbolo : simbolo + (l === 'en' ? '' : ' ') + s;
        }
        return s;
    };
})();
const __nome =(r,c) => { let s=''; for(let n=c+1;n>0;n=Math.floor((n-1)/26)) s=String.fromCharCode(65+(n-1)%26)+s; return s+(r+1); };
// O tipo de erro vive no JavaScript da macro, mas os códigos validados
// e a avaliação das fórmulas continuam pertencendo ao core Rust.
const __codigosErro = new Set(['#DIV/0!','#VALUE!','#REF!','#NAME?','#ERROR!','#NUM!','#N/A','#SPILL!','#CIRCULAR!','#CALCULANDO!']);
class __CellError {
    constructor(code,detail) {
        if(!__codigosErro.has(code)) throw new Error('Código de erro de planilha inválido: '+code);
        this.code=code; this.detail=detail;
        this.__ftCellError=code;
    }
    toString(){return this.code;}
}
const __ftRevive = v => {
    if(Array.isArray(v)) return v.map(__ftRevive);
    if(v!==null && typeof v==='object' && __codigosErro.has(v.__ftCellError))
        return new __CellError(v.__ftCellError,v.detail);
    return v;
};
function __sheet(i) {
    const range = address => {
        const a = __ft('area',[i,address]);
        const x = {
            address:__nome(a[0],a[1])+(a[0]===a[2]&&a[1]===a[3]?'':':'+__nome(a[2],a[3])),
            rows:a[2]-a[0]+1, columns:a[3]-a[1]+1,
            get value(){return __ftRevive(__ft('get',[i,address]));},
            set value(v){__ft('set',[i,address,v]);},
            get text(){return __ft('raw',[i,address]);},
            set text(v){__ft('set',[i,address,v]);},
            get displayText(){return __ft('getText',[i,address]);},
            getText(){return __ft('getText',[i,address]);},
            getAddress(){return __ft('getAddress',[i,address]);},
            values:()=>__ftRevive(__ft('values',[i,address])),
            formulas:()=>__ft('formulas',[i,address]),
            set(v){__ft('set',[i,address,v]);return x;},
            setFormula(f){__ft('setFormula',[i,address,f]);return x;},
            setFormulaR1C1(f){__ft('setFormulaR1C1',[i,address,f]);return x;},
            setNumberFormat(mask){__ft('setNumberFormat',[i,address,mask]);return x;},
            setFontFamily(name){__ft('setFontFamily',[i,address,name]);return x;},
            setFontSize(px){__ft('setFontSize',[i,address,px]);return x;},
            setBold(v=true){__ft('setBold',[i,address,v]);return x;},
            setItalic(v=true){__ft('setItalic',[i,address,v]);return x;},
            setUnderline(v='single'){__ft('setUnderline',[i,address,v]);return x;},
            setStrikethrough(v=true){__ft('setStrikethrough',[i,address,v]);return x;},
            setFontColor(color){__ft('setFontColor',[i,address,color]);return x;},
            setBackground(color){__ft('setBackground',[i,address,color]);return x;},
            setAlignHorizontal(v){__ft('setAlignHorizontal',[i,address,v]);return x;},
            setAlignVertical(v){__ft('setAlignVertical',[i,address,v]);return x;},
            setWrapText(v=true){__ft('setWrapText',[i,address,v]);return x;},
            setTextRotation(deg){__ft('setTextRotation',[i,address,deg]);return x;},
            setHyperlink(url,text=undefined){__ft('setHyperlink',[i,address,url,text]);return x;},
            setBorder(opts){__ft('setBorder',[i,address,opts]);return x;},
            setOutline(opts={}){__ft('setOutline',[i,address,opts]);return x;},
            copy(dest,opts={}){__ft('copy',[i,address,dest,opts]);return x;},
            move(dest){__ft('move',[i,address,dest]);return x;},
            deleteCells(shift='up'){__ft('deleteCells',[i,address,shift]);return x;},
            insertCells(shift='down'){__ft('insertCells',[i,address,shift]);return x;},
            autoFill(dest){return __ft('autoFill',[i,address,dest]);},
            sort(keys,opts={}){__ft('sort',[i,address,keys,opts]);return x;},
            removeDuplicates(cols,opts={}){return __ft('removeDuplicates',[i,address,cols,opts]);},
            textToColumns(opts={}){return __ft('textToColumns',[i,address,opts]);},
            setDataValidation(rule){__ft('setDataValidation',[i,address,rule]);return x;},
            clearDataValidation(){__ft('clearDataValidation',[i,address]);return x;},
            addConditionalFormat(o){__ft('addConditionalFormat',[i,address,o]);return x;},
            addColorScale(o){__ft('addColorScale',[i,address,o]);return x;},
            addDataBar(o={}){__ft('addDataBar',[i,address,o]);return x;},
            clearConditionalFormats(){__ft('clearConditionalFormats',[i,address]);return x;},
            clearContents(){__ft('clearContents',[i,address]);return x;},
            clearFormats(){__ft('clearFormats',[i,address]);return x;},
            replace(oldText,newText,opts={}){return __ft('replace',[i,address,oldText,newText,opts]);},
            find(text,opts={}){return __ft('find',[i,text,{...opts,range:address}]);},
            specialCells(tipo){return __ft('specialCells',[i,address,tipo]);},
            clear(){__ft('clear',[i,address]);return x;},
            select(){__sel[i]=x.address;__ft('ativar',[i]);return x;},
            activate(){return x.select();},
            forEach(fn){const values=x.values();values.forEach((row,r)=>row.forEach((v,c)=>fn(v,r,c,__nome(a[0]+r,a[1]+c))));return x;},
            map(fn){x.forEach((v,r,c,addr)=>{const next=fn(v,r,c);if(next!==undefined)__ft('set',[i,addr,next]);});return x;}
        }; return x;
    };
    const sheet = {range, cell:(r,c)=>typeof r==='string'?range(r):range(__nome(r-1,c-1)),
        get lastRow(){return __ft('lastRow',[i]);},
        get lastColumn(){return __ft('lastColumn',[i]);},
        address:(r,c)=>__ft('address',[i,r,c]),
        getLastRow:col=>__ft('getLastRow',[i,col===undefined?'A':col]),
        getLastColumn:r=>__ft('getLastColumn',[i,r===undefined?1:r]),
        // Tamanhos das seleções são metadados, sem varredura de células.
        rowCount:a=>__ft('rowCount',[i,a]),
        columnCount:a=>__ft('columnCount',[i,a]),
        setRowHeight:(r,px)=>{__ft('setRowHeight',[i,r,px]);return sheet;},
        setColumnWidth:(c,px)=>{__ft('setColumnWidth',[i,c,px]);return sheet;},
        hideRow:(r,hide=true)=>{__ft('hideRow',[i,r,hide]);return sheet;},
        hideColumn:(c,hide=true)=>{__ft('hideColumn',[i,c,hide]);return sheet;},
        getDataRange:()=>{
            const endereco=__ft('getDataRange',[i]);
            return endereco===null?null:range(endereco);
        },
        getRegion:a=>range(__ft('getRegion',[i,a])),
        merge:a=>{__ft('merge',[i,a]);return sheet;},
        unmerge:a=>{__ft('unmerge',[i,a]);return sheet;},
        isMerged:a=>__ft('isMerged',[i,a]),
        offset:(a,dr,dc)=>range(__ft('offset',[i,a,dr,dc])),
        resize:(a,rows,cols)=>range(__ft('resize',[i,a,rows,cols])),
        columnOf:(a,n)=>range(__ft('columnOf',[i,a,n])),
        rowOf:(a,n)=>range(__ft('rowOf',[i,a,n])),
        get:a=>range(a).value,
        getText:a=>__ft('getText',[i,a]),
        getAddress:r=>{
            if(r!==null && typeof r==='object' && 'address' in r) return r.address;
            if(r!==null && typeof r==='object' && 'r0' in r){
                if(!['r0','c0','r1','c1'].every(k=>Number.isInteger(r[k]))
                    || r.r0<0 || r.r1<0 || r.c0<0 || r.c1<0
                    || r.r0>=100000000 || r.r1>=100000000
                    || r.c0>=16384 || r.c1>=16384) throw new Error('Retângulo inválido');
                r=__nome(r.r0,r.c0)+':'+__nome(r.r1,r.c1);
            }
            return __ft('getAddress',[i,r]);
        },
        set:(a,v)=>{range(a).set(v);return sheet;},
        setFormula:(a,v)=>{range(a).setFormula(v);return sheet;},
        setFormulaR1C1:(a,v)=>{range(a).setFormulaR1C1(v);return sheet;},
        setNumberFormat:(a,v)=>{range(a).setNumberFormat(v);return sheet;},
        setFontFamily:(a,name)=>{range(a).setFontFamily(name);return sheet;},
        setFontSize:(a,px)=>{range(a).setFontSize(px);return sheet;},
        setBold:(a,v=true)=>{range(a).setBold(v);return sheet;},
        setItalic:(a,v=true)=>{range(a).setItalic(v);return sheet;},
        setUnderline:(a,v='single')=>{range(a).setUnderline(v);return sheet;},
        setStrikethrough:(a,v=true)=>{range(a).setStrikethrough(v);return sheet;},
        setFontColor:(a,color)=>{range(a).setFontColor(color);return sheet;},
        setBackground:(a,color)=>{range(a).setBackground(color);return sheet;},
        setAlignHorizontal:(a,v)=>{range(a).setAlignHorizontal(v);return sheet;},
        setAlignVertical:(a,v)=>{range(a).setAlignVertical(v);return sheet;},
        setWrapText:(a,v=true)=>{range(a).setWrapText(v);return sheet;},
        setTextRotation:(a,deg)=>{range(a).setTextRotation(deg);return sheet;},
        setHyperlink:(a,url,text=undefined)=>{range(a).setHyperlink(url,text);return sheet;},
        setBorder:(a,opts)=>{range(a).setBorder(opts);return sheet;},
        setOutline:(a,opts={})=>{range(a).setOutline(opts);return sheet;},
        copy:(a,b,opts={})=>{range(a).copy(b,opts);return sheet;},
        move:(a,b)=>{range(a).move(b);return sheet;},
        deleteCells:(a,shift='up')=>{range(a).deleteCells(shift);return sheet;},
        insertCells:(a,shift='down')=>{range(a).insertCells(shift);return sheet;},
        insertRow:(r,n=1)=>{__ft('insertRow',[i,r,n]);return sheet;},
        deleteRow:(r,n=1)=>{__ft('deleteRow',[i,r,n]);return sheet;},
        insertColumn:(c,n=1)=>{__ft('insertColumn',[i,c,n]);return sheet;},
        deleteColumn:(c,n=1)=>{__ft('deleteColumn',[i,c,n]);return sheet;},
        autoFill:(a,b)=>range(a).autoFill(b),
        sort:(a,keys,opts={})=>{range(a).sort(keys,opts);return sheet;},
        removeDuplicates:(a,cols,opts={})=>range(a).removeDuplicates(cols,opts),
        textToColumns:(a,opts={})=>range(a).textToColumns(opts),
        setDataValidation:(a,rule)=>{range(a).setDataValidation(rule);return sheet;},
        clearDataValidation:a=>{range(a).clearDataValidation();return sheet;},
        addConditionalFormat:(a,o)=>{range(a).addConditionalFormat(o);return sheet;},
        addColorScale:(a,o)=>{range(a).addColorScale(o);return sheet;},
        addDataBar:(a,o={})=>{range(a).addDataBar(o);return sheet;},
        clearConditionalFormats:a=>{range(a).clearConditionalFormats();return sheet;},
        clearContents:a=>{range(a).clearContents();return sheet;},
        clearFormats:a=>{range(a).clearFormats();return sheet;},
        replace:(a,oldText,newText,opts={})=>range(a).replace(oldText,newText,opts),
        find:(text,opts={})=>__ft('find',[i,text,opts]),
        specialCells:(a,tipo)=>range(a).specialCells(tipo),
        intersect:(a,b)=>{const addr=__ft('intersect',[i,a,b]);return addr===null?null:range(addr);},
        union:(a,b)=>range(__ft('union',[i,a,b])),
        clear:a=>{range(a).clear();return sheet;},
        getRaw:a=>range(a).text,
        read:a=>range(a).values(),write:(a,v)=>{range(a).set(v);return sheet;},
        value:a=>range(a).value,
        // Seleção da macro: não move a tela, só responde a selection()/activeCell().
        select:a=>range(a).select(),
        selection:()=>range(__sel[i]||'A1'),
        activeCell:()=>range((__sel[i]||'A1').split(':')[0]),
        activate:()=>{__ft('ativar',[i]);return sheet;},
        // Arquivos: só anotados; o aplicativo grava se a macro terminar bem.
        exportPdf:path=>{__ft('exportarPdf',[i,String(path)]);return sheet;},
        saveAs:path=>{__ft('salvarAbas',[[i+1],String(path)]);return sheet;},
        defineFunction:(nome)=>{log('Função '+nome+' definida só nesta macro: as fórmulas da planilha ainda não a usam.');},
        status:m=>app.status(m),
        log, name:__ft('nomes',[])[i]};
    sheet.cells=sheet.cell; sheet.worksheets=wb.sheet; sheet.sheets=wb.sheet; return sheet;
}
const __sel = {};
const alert = t => { __ft('alerta',[t===undefined?'':String(t)]); };
const confirm = t => __ft('confirmar',[t===undefined?'':String(t)]);
const wb = {
    activate:name=>wb.sheet(name).activate(),
    get activeSheet(){return __sheet(__ft('abaAtiva',[]));},
    addName:(name,ref)=>{__ft('definirNome',[String(name),String(ref),__ft('abaAtiva',[])]);},
    get name(){return __ft('arquivo',[]).nome;},
    get path(){return __ft('arquivo',[]).pasta;},
    get fullName(){return __ft('arquivo',[]).completo;},
    // Uma pasta nova só com estas abas (nomes, números do 1 ou abas), .xlsx ou .fshz.
    saveSheetsAs:(names,path)=>{__ft('salvarAbas',[(Array.isArray(names)?names:[names])
        .map(n=>n!==null&&typeof n==='object'?n.name:n),String(path)]);},
    sheet(name){
    const names=__ft('nomes',[]);
    const i=typeof name==='number'
        ? (Number.isInteger(name)?name-1:-1)
        : __ft('indiceAba',[String(name).trim()]);
    if(i===null||i<0||i>=names.length)throw new Error('Aba inexistente: '+name);
    return __sheet(i);
},sheetNames:()=>__ft('nomes',[]),get sheetCount(){return __ft('nomes',[]).length;}};
const app = {
    batch: fn => fn(),
    // Eventos e redesenho da tela do Excel: sem efeito aqui.
    setEventsEnabled: () => {},
    setScreenUpdating: () => {},
    rgb: (r,g,b) => __ft('appRgb',[r,g,b]),
    vbaColorToHex: n => __ft('appVbaColorToHex',[n]),
    isError: v => v instanceof __CellError,
    // api-aplicacao.ts: mantém a sintaxe pública JS; validações/estado no Rust.
    error: (code,detail=undefined) => __ftRevive(__ft('appError',
        [String(code),detail===undefined?null:String(detail)])),
    status: message => __ft('appStatus',[String(message)]),
    calculate: () => __ft('appCalculate',[]),
    fn: Object.freeze({
        sum: values => __ft('worksheetSum',[values]),
        count: values => __ft('worksheetCount',[values]),
        countA: values => __ft('worksheetCountA',[values]),
        max: values => __ft('worksheetMax',[values]),
        min: values => __ft('worksheetMin',[values]),
        average: values => __ftRevive(__ft('worksheetAverage',[values])),
        median: values => __ftRevive(__ft('worksheetMedian',[values])),
        averageIf: (values,criterion,amounts=undefined) => __ftRevive(
            amounts===undefined
                ? __ft('worksheetAverageIf',[values,criterion])
                : __ft('worksheetAverageIf',[values,criterion,amounts])
        ),
        vlookup: (key,table,column,exact=true) => __ftRevive(
            __ft('worksheetVLookup',[key,table,column,exact])),
        hlookup: (key,table,row,exact=true) => __ftRevive(
            __ft('worksheetHLookup',[key,table,row,exact])),
        match: (value,list,type=0) => __ftRevive(
            __ft('worksheetMatch',[value,list,type])),
        index: (matrix,row,column=undefined) => __ftRevive(
            column===undefined
                ? __ft('worksheetIndex',[matrix,row])
                : __ft('worksheetIndex',[matrix,row,column])),
        countIf: (values,criterion) => __ft('worksheetCountIf',[values,criterion]),
        sumIf: (values,criterion,amounts=undefined) =>
            amounts===undefined
                ? __ft('worksheetSumIf',[values,criterion])
                : __ft('worksheetSumIf',[values,criterion,amounts])
    })
};
"#;

mod posicoes;
#[cfg(test)]
mod testes_formato;
#[cfg(test)]
mod testes_estrutura;
#[cfg(test)]
mod testes_aplicacao;
#[cfg(test)]
mod testes_busca;
#[cfg(test)]
mod testes_validacoes;
#[cfg(test)]
mod testes_condicionais;
#[cfg(test)]
mod testes_pasta;

#[cfg(test)]
mod testes;
