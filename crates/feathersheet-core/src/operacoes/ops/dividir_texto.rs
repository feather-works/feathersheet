//! Dividir texto em colunas.

use std::collections::HashMap;
use crate::celulas::estilo_parcial::{nome_do_formato, Campo, EstiloParcial};
use crate::celulas::input::{date_to_serial, parse_number_str};
use crate::celulas::refs::Rect;
use crate::celulas::style::NumberFormat as FormatoDeEstilo;
use crate::celulas::values::{to_text, Value};
use crate::planilha::sheet::Sheet;
use super::comum::trim_js;

/// Como dividir o texto no assistente de "dividir texto em colunas" — ao
/// estilo do Excel: um ou mais delimitadores marcados ao mesmo tempo
/// (`delimitadores`, cada um um token — geralmente um caractere, mas
/// "Outros:" pode ser mais de um), a opção de tratar delimitadores vizinhos
/// como um só (`;;` não vira campo vazio no meio), e um qualificador de
/// texto opcional (`"texto, com vírgula"` não divide na vírgula de dentro).
#[derive(Debug, Clone, Default)]
pub struct OpcoesDivisao {
    pub delimitadores: Vec<String>,
    pub consecutivos_como_um: bool,
    pub qualificador_texto: Option<char>,
}

/// Conta os campos sem alocar Vec<String>, inclusive quando há
/// delimitadores consecutivos. O assistente usa isto em sua primeira
/// passagem por 1M linhas (validação); a escrita divide somente depois.
pub fn contar_colunas_divisao(texto: &str, opcoes: &OpcoesDivisao) -> usize {
    if let [token] = opcoes.delimitadores.as_slice() {
        if !token.is_empty()
            && !opcoes.qualificador_texto.is_some_and(|q| texto.contains(q))
        {
            return texto.split(token.as_str())
                .filter(|parte| !opcoes.consecutivos_como_um || !parte.is_empty())
                .count().max(1);
        }
    }
    dividir_com_opcoes(texto, opcoes).len()
}

/// Divide um texto pelas opções acima. Fora do `planejar_divisao` para dar
/// para testar sozinho — é a parte que não sabe nada de planilha.
pub fn dividir_com_opcoes(texto: &str, opcoes: &OpcoesDivisao) -> Vec<String> {
    // Caminho mais comum na Ribbon: um delimitador literal, sem aspas
    // qualificadoras nem compactação de campos vazios. Usa o split nativo
    // em vez de testar todos os tokens em cada caractere Unicode.
    // O caso sem delimitador também evita o laço de tokenização.
    if let [separador] = opcoes.delimitadores.as_slice() {
        let separador = separador.as_str();
        // Padrão do assistente: aspas ativadas e separadores consecutivos
        // agrupados. Se a linha não contém o qualificador, o split nativo
        // trata AMBAS as variantes (com/sem agrupamento) corretamente.
        let tem_qualificador = opcoes.qualificador_texto
            .is_some_and(|q| texto.contains(q));
        if !separador.is_empty() && !tem_qualificador {
            if !texto.contains(separador) { return vec![texto.to_owned()]; }
            let mut partes: Vec<String> = texto.split(separador)
                .filter(|parte| !opcoes.consecutivos_como_um || !parte.is_empty())
                .map(str::to_owned)
                .collect();
            if partes.is_empty() { partes.push(String::new()); }
            return partes;
        }
    }
    let mut campos = Vec::new();
    let mut atual = String::new();
    let mut dentro_qualificador = false;
    let mut resto = texto;
    while !resto.is_empty() {
        if let Some(q) = opcoes.qualificador_texto {
            if let Some(sem_q) = resto.strip_prefix(q) {
                if dentro_qualificador && sem_q.starts_with(q) {
                    // `""` dentro de um campo qualificado é um `"` literal.
                    atual.push(q);
                    resto = &sem_q[q.len_utf8()..];
                    continue;
                }
                dentro_qualificador = !dentro_qualificador;
                resto = sem_q;
                continue;
            }
        }
        if !dentro_qualificador {
            let achado = opcoes.delimitadores.iter().filter(|d| !d.is_empty()).find(|d| resto.starts_with(d.as_str()));
            if let Some(tok) = achado {
                campos.push(std::mem::take(&mut atual));
                resto = &resto[tok.len()..];
                continue;
            }
        }
        let mut chars = resto.chars();
        atual.push(chars.next().expect("resto não está vazio aqui"));
        resto = chars.as_str();
    }
    campos.push(atual);
    if opcoes.consecutivos_como_um {
        campos.retain(|s| !s.is_empty());
        if campos.is_empty() {
            campos.push(String::new());
        }
    }
    campos
}

/// A metade que lê do `splitTextToColumns`: as linhas com texto e os
/// pedaços. `None` quando nada tem mais de um pedaço. Divide o valor
/// mostrado (uma fórmula divide o resultado), como o TypeScript.
pub fn planejar_divisao(
    area: Rect,
    opcoes: &OpcoesDivisao,
    max_row: u32,
    mut valor: impl FnMut(u32, u32) -> Value,
) -> Option<Vec<(u32, Vec<String>)>> {
    let r = area.normalized();
    if opcoes.delimitadores.iter().all(String::is_empty) {
        return None;
    }
    let max_r = r.r1.min(max_row);
    let mut linhas = Vec::new();
    let mut mais_larga = 0;
    for row in r.r0..=max_r {
        let texto = to_text(&valor(row, r.c0));
        if texto.is_empty() {
            continue;
        }
        let partes = dividir_com_opcoes(&texto, opcoes);
        mais_larga = mais_larga.max(partes.len());
        linhas.push((row, partes));
    }
    (mais_larga > 1).then_some(linhas)
}

/// O tipo escolhido, no assistente de "dividir texto em colunas", para uma
/// coluna resultante — ou o pedido para não importá-la.
#[derive(Debug, Clone, PartialEq)]
pub enum TipoColunaDivisao {
    /// Detecta sozinho, como a digitação normal (mesma regra do
    /// [`crate::celulas::input::parse_literal`]).
    Geral,
    Texto,
    Numero,
    Data(FormatoDeData),
    Porcentagem,
    /// A coluna não é escrita — nem o texto, nem vazio: intocada.
    Pular,
}

/// A ordem dos três campos de uma data digitada. A digitação normal adivinha
/// (dia vem primeiro só se o mês não bate); aqui a pessoa escolhe, porque
/// "01/02/03" sozinho é ambíguo demais para adivinhar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoDeData {
    DiaMesAno,
    MesDiaAno,
    AnoMesDia,
}

/// Lê uma data na ordem escolhida (separador `/`, `-` ou `.`). `None` quando
/// não são três números, ou quando dia/mês caem fora da faixa.
pub(super) fn parse_data_com_formato(texto: &str, formato: FormatoDeData) -> Option<f64> {
    let partes: Vec<&str> = texto.trim().split(['/', '-', '.']).collect();
    let [a, b, c] = partes[..] else { return None };
    let (a, b, c) = (a.parse::<i32>().ok()?, b.parse::<i32>().ok()?, c.parse::<i32>().ok()?);
    let (dia, mes, ano) = match formato {
        FormatoDeData::DiaMesAno => (a, b, c),
        FormatoDeData::MesDiaAno => (b, a, c),
        FormatoDeData::AnoMesDia => (c, b, a),
    };
    if !(1..=31).contains(&dia) || !(1..=12).contains(&mes) {
        return None;
    }
    let ano_completo = if ano < 100 { if ano >= 30 { 1900 + ano } else { 2000 + ano } } else { ano };
    Some(date_to_serial(ano_completo, mes, dia))
}

/// O valor de um pedaço já dividido, segundo o tipo escolhido. Quando o texto
/// não cabe no tipo pedido (ex.: "abc" como Número), vira texto — como o
/// Excel faz, em vez de recusar a divisão inteira. `Geral`/`Pular` não
/// chamam esta função (o chamador os trata antes).
pub(super) fn valor_para_tipo(texto: &str, tipo: &TipoColunaDivisao) -> Value {
    match tipo {
        TipoColunaDivisao::Texto => Value::Text(texto.to_string()),
        TipoColunaDivisao::Numero => {
            parse_number_str(texto).map(Value::Number).unwrap_or_else(|| Value::Text(texto.to_string()))
        }
        TipoColunaDivisao::Porcentagem => {
            let sem_sinal = texto.trim().trim_end_matches('%').trim();
            parse_number_str(sem_sinal)
                .map(|n| Value::Number(n / 100.0))
                .unwrap_or_else(|| Value::Text(texto.to_string()))
        }
        TipoColunaDivisao::Data(formato) => {
            parse_data_com_formato(texto, *formato).map(Value::Number).unwrap_or_else(|| Value::Text(texto.to_string()))
        }
        TipoColunaDivisao::Geral | TipoColunaDivisao::Pular => {
            unreachable!("Geral e Pular são tratados antes de chamar valor_para_tipo")
        }
    }
}

/// O formato de exibição que o tipo escolhido pede, ou `None` quando o
/// padrão (`Auto`) já serve — é o caso de Texto e Número, que se mostram
/// sozinhos sem precisar de um formato guardado.
pub(super) fn estilo_para_tipo(sheet: &mut Sheet, tipo: &TipoColunaDivisao) -> Option<usize> {
    let formato = match tipo {
        TipoColunaDivisao::Data(_) => FormatoDeEstilo::Date,
        TipoColunaDivisao::Porcentagem => FormatoDeEstilo::Percent,
        _ => return None,
    };
    let mut parcial = EstiloParcial::new();
    parcial.insert("format".into(), Campo::Texto(nome_do_formato(formato).unwrap_or("auto").into()));
    sheet.registrar_parcial(parcial)
}

/// Um campo com tipo escolhido pela pessoa (Texto, Número, Data, %): o
/// mesmo que o [`aplicar_divisao`] faz para essas colunas.
pub fn gravar_campo_tipado(sheet: &mut Sheet, row: u32, col: u32, texto: &str, tipo: &TipoColunaDivisao) {
    sheet.set_cell_valor(row, col, valor_para_tipo(texto, tipo));
    if let Some(idx) = estilo_para_tipo(sheet, tipo) {
        sheet.set_style_idx(row, col, idx);
    }
}

/// A receita de um Texto para Colunas na mesma aba: de onde lê, para onde
/// escreve e como corta. É o que o refazer guarda no lugar das células.
#[derive(Debug, Clone)]
pub struct PlanoDivisao {
    pub col_origem: u32,
    pub r0: u32,
    pub r1: u32,
    pub dst_row: u32,
    pub dst_col: u32,
    /// Quantas colunas o resultado ocupa (a validação mede).
    pub colunas: u32,
    /// `Some(n)`: largura fixa de `n` caracteres; `None`: delimitado.
    pub largura: Option<usize>,
    pub opcoes: OpcoesDivisao,
    pub tipos: Vec<TipoColunaDivisao>,
    /// Resultado das fórmulas da coluna de origem, lido antes de escrever.
    pub formulas: HashMap<u32, String>,
}

impl PlanoDivisao {
    /// `[r0, c0, r1, c1]` do destino.
    pub fn area(&self) -> [u32; 4] {
        [self.dst_row, self.dst_col, self.dst_row + (self.r1 - self.r0),
            (self.dst_col + self.colunas.max(1) - 1).min(crate::celulas::refs::MAX_COLS - 1)]
    }

    /// O texto a dividir na linha `r` da origem (vazio: nada a fazer).
    pub fn texto_origem(&self, sheet: &Sheet, r: u32) -> String {
        if let Some(f) = self.formulas.get(&r) { return f.clone(); }
        match sheet.texto_cru_divisao(r, self.col_origem) {
            Some(raw) if !raw.starts_with('=') => raw.to_owned(),
            _ => match sheet.get_value(r, self.col_origem) {
                Value::Empty => String::new(),
                Value::Text(s) => s,
                Value::Number(n) => format!("{n}"),
                Value::Bool(b) => b.to_string(),
                Value::Error(e) => e.to_string(),
            },
        }
    }

    /// Corta e analisa uma linha — não lê a planilha, roda em qualquer thread.
    pub fn preparar(&self, texto: &str) -> Vec<crate::planilha::sheet::edicao::CampoDivisao> {
        use crate::planilha::sheet::edicao::CampoDivisao;
        match self.largura {
            Some(largura) => dividir_largura_fixa(texto, largura).iter()
                .map(|p| CampoDivisao::novo(trim_js(p))).collect(),
            None => dividir_com_opcoes(texto, &self.opcoes).iter()
                .map(|p| CampoDivisao::novo(trim_js(p))).collect(),
        }
    }

    /// [`Self::preparar`] de várias linhas usando todos os núcleos. Devolve
    /// já com a linha de destino.
    pub fn preparar_linhas(&self, linhas: Vec<(u32, String)>)
        -> Vec<(u32, Vec<crate::planilha::sheet::edicao::CampoDivisao>)> {
        let fazer = |fatia: &[(u32, String)]| -> Vec<_> {
            fatia.iter().filter(|(_, t)| !t.is_empty())
                .map(|(r, t)| (self.dst_row + (r - self.r0), self.preparar(t))).collect()
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let threads = crate::paralelismo::nucleos();
            if threads > 1 && linhas.len() > 1024 {
                let tamanho = linhas.len().div_ceil(threads);
                return std::thread::scope(|escopo| {
                    let alcas: Vec<_> = linhas.chunks(tamanho)
                        .map(|fatia| escopo.spawn(move || fazer(fatia))).collect();
                    alcas.into_iter().flat_map(|a| a.join().expect("thread da divisão")).collect()
                });
            }
        }
        fazer(&linhas)
    }
}

/// Largura fixa por caracteres (não bytes): "ação" de 2 em 2 é "aç", "ão".
pub fn dividir_largura_fixa(texto: &str, largura: usize) -> Vec<String> {
    let largura = largura.max(1);
    let mut partes = Vec::new();
    let (mut inicio, mut tamanho) = (0, 0);
    for (indice, _) in texto.char_indices() {
        if tamanho == largura {
            partes.push(texto[inicio..indice].to_owned());
            inicio = indice;
            tamanho = 0;
        }
        tamanho += 1;
    }
    if inicio < texto.len() {
        partes.push(texto[inicio..].to_owned());
    }
    partes
}

/// Começa a divisão: empilha na transação aberta o desfazer leve (foto só do
/// que já existia na área) e a receita do refazer.
pub fn iniciar_divisao(sheet: &mut Sheet, plano: std::sync::Arc<PlanoDivisao>)
    -> crate::planilha::sheet::edicao::GravadorDivisao {
    use crate::planilha::sheet::edicao::Refazer;
    let receita = std::sync::Arc::clone(&plano);
    let refazer = Refazer(std::sync::Arc::new(move |s: &mut Sheet| executar_divisao(s, &receita)));
    sheet.iniciar_divisao(plano.area(), plano.tipos.clone(), Some(refazer))
}

/// Quantos textos diferentes a divisão deve criar, por uma amostra de linhas da origem
/// (`amostra`, uns 512 textos espalhados; `total` = linhas da divisão). Serve para reservar
/// o pool de uma vez ([`crate::celulas::pool::StringPool::reservar`]). Coluna quase toda
/// distinta (id, nome) vale a fração vezes o total; coluna de categoria (poucos valores que
/// se repetem, "Vitória") vale só os distintos, dobrados. Número não conta: não vai para o
/// pool. Pura: roda fora da thread da janela.
pub fn estimar_textos_novos(plano: &PlanoDivisao, amostra: &[String], total: usize) -> usize {
    use crate::Value;
    let linhas = amostra.iter().filter(|t| !t.is_empty()).count();
    let mut distintos: Vec<std::collections::HashSet<String>> = Vec::new();
    for texto in amostra.iter().filter(|t| !t.is_empty()) {
        for (c, campo) in plano.preparar(texto).into_iter().enumerate() {
            let texto = match &campo.rapido {
                Some(r) => matches!(r.valor, Value::Text(_)),
                None => !campo.raw.is_empty(),
            };
            if !texto { continue; }
            if distintos.len() <= c { distintos.resize_with(c + 1, Default::default); }
            distintos[c].insert(campo.raw);
        }
    }
    let estimativa: usize = distintos.iter().map(|d| {
        if d.len() * 2 < linhas { d.len() * 2 } else { d.len().saturating_mul(total) / linhas.max(1) }
    }).sum();
    // Teto: 64 milhões de textos (~2,6 GB de índice) — além disso, deixa o mapa crescer sozinho.
    estimativa.min(64 << 20)
}

/// A divisão inteira de uma vez, sem histórico — é o refazer.
pub fn executar_divisao(sheet: &mut Sheet, plano: &PlanoDivisao) {
    let mut g = sheet.iniciar_divisao(plano.area(), plano.tipos.clone(), None);
    let mut r = plano.r0;
    while r <= plano.r1 {
        let fim = r.saturating_add(65_535).min(plano.r1);
        let textos: Vec<(u32, String)> = (r..=fim).map(|l| (l, plano.texto_origem(sheet, l))).collect();
        let linhas = plano.preparar_linhas(textos);
        sheet.gravar_divisao(&mut g, plano.dst_col, &linhas);
        if fim == u32::MAX { break; }
        r = fim + 1;
    }
    if sheet.formulas.size() > 0 { sheet.invalidar_todos_calculos(); }
}

/// A metade que escreve: cada pedaço, aparado, numa coluna, no tipo pedido
/// para ela (ou detectado sozinho, quando `tipos` é `None` ou mais curto que
/// a linha — mesmo comportamento de antes do assistente ganhar tipo por
/// coluna). Devolve quantas linhas (todas as que tinham texto, como o
/// TypeScript conta).
pub fn aplicar_divisao(sheet: &mut Sheet, c0: u32, linhas: &[(u32, Vec<String>)], tipos: Option<&[TipoColunaDivisao]>) -> usize {
    sheet.abrir_transacao();
    for (row, partes) in linhas {
        for (i, parte) in partes.iter().enumerate() {
            let tipo = tipos.and_then(|t| t.get(i)).unwrap_or(&TipoColunaDivisao::Geral);
            if matches!(tipo, TipoColunaDivisao::Pular) {
                continue;
            }
            let col = c0 + i as u32;
            let texto = trim_js(parte);
            if matches!(tipo, TipoColunaDivisao::Geral) {
                // Lote já está em transação: parse único, sem limpar e
                // regravar a mesma célula, mantendo undo/redo idênticos.
                sheet.set_cell_divisao(*row, col, texto);
                continue;
            }
            sheet.set_cell_valor(*row, col, valor_para_tipo(texto, tipo));
            if let Some(idx) = estilo_para_tipo(sheet, tipo) {
                sheet.set_style_idx(*row, col, idx);
            }
        }
    }
    sheet.fechar_transacao();
    linhas.len()
}

/// `splitTextToColumns`, lendo o `Sheet` sozinho — sem tipo por coluna
/// (detecta tudo sozinho, como sempre foi), um só separador literal (sem
/// consecutivos-como-um nem qualificador — é o caminho simples de teste;
/// o assistente de verdade usa `planejar_divisao` com `OpcoesDivisao`).
pub fn split_text_to_columns(sheet: &mut Sheet, area: Rect, separador: &str) -> usize {
    let max_row = sheet.max_row_ts();
    let opcoes = OpcoesDivisao { delimitadores: vec![separador.to_string()], ..Default::default() };
    let linhas = {
        let s = &*sheet;
        planejar_divisao(area.clone(), &opcoes, max_row, |r, c| s.get_value(r, c))
    };
    match linhas {
        Some(l) => aplicar_divisao(sheet, area.normalized().c0, &l, None),
        None => 0,
    }
}

