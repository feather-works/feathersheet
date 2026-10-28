//! Carga de arquivo em lote: o trabalho de cada célula que não depende da
//! aba (analisar o literal, a fórmula e o padrão dela) é feito antes, em
//! várias threads ([`CelulaPreparada::preparar`]); a gravação, numa só
//! ([`Sheet::carregar_preparada`]), pula o que só serve à edição: foto para o
//! desfazer, limpar o que a célula tinha e propagar "sujo" (a aba acabou de
//! nascer, nada foi calculado ainda).
//!
//! Era o `apply` célula por célula: 51 s dos 73 s de abrir um XLSX de
//! 13 milhões de células (13/10/2026).
//!
//! **Memória entre threads.** Liberar na thread que grava o que foi alocado
//! nas outras fazia o alocador disputar trava a cada célula (gravar levava
//! 28,9 s em vez de 11,4 s). Por isso: o texto das células chega num texto
//! só por lote ([`LoteBruto`]), a fórmula que vai para a coluna empacotada
//! nem ganha árvore (o modelo do padrão resolve), o padrão é partilhado
//! ([`Padroes`]), e o que sobra vai para [`Sobra`], solto por quem chamou.

use std::collections::HashMap;
use std::sync::Arc;

use crate::celulas::estilo_parcial::EstiloParcial;
use crate::celulas::refs::{key, MAX_COLS, MAX_ROWS};
use crate::planilha::column_store::CHUNK_SHIFT;
use crate::celulas::values::{CellError, ErrorCode, Value};
use crate::formula::ast::Node;
use crate::formula::formula_pattern::pattern_for;
use crate::formula::formula_table::FormulaTable;
use crate::formula::parser::parse_formula;
use crate::planilha::sheet_deps::is_volatile;

use super::edicao::{round_trips, Snap};
use super::{le_de_fora, pattern_check, CellData, Sheet};

/// Células de um lote com o texto de todas num `String` só.
#[derive(Debug, Default)]
pub struct LoteBruto {
    pub texto: String,
    /// Linha, coluna, estilo do arquivo, valor guardado e onde o texto da
    /// célula está em `texto`.
    pub celulas: Vec<(u32, u32, usize, Option<Value>, (u32, u32))>,
}

impl LoteBruto {
    pub fn com_capacidade(celulas: usize) -> Self {
        LoteBruto { texto: String::with_capacity(celulas * 12), celulas: Vec::with_capacity(celulas) }
    }

    pub fn len(&self) -> usize {
        self.celulas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.celulas.is_empty()
    }

    pub fn push(&mut self, row: u32, col: u32, raw: &str, estilo: usize, guardado: Option<Value>) {
        let inicio = self.texto.len() as u32;
        self.texto.push_str(raw);
        self.celulas.push((row, col, estilo, guardado, (inicio, self.texto.len() as u32)));
    }
}

/// O lote pronto para gravar: o mesmo texto e as células analisadas.
#[derive(Debug, Default)]
pub struct LotePreparado {
    pub texto: String,
    pub celulas: Vec<CelulaPreparada>,
}

/// Padrões já vistos por uma thread de preparo: o texto partilhado e se
/// ele cabe na coluna empacotada (decidido uma vez por padrão).
#[derive(Default)]
pub struct Padroes(HashMap<String, (Arc<str>, bool)>);

/// O que já se sabe da célula antes de ela entrar na aba.
#[derive(Debug)]
pub enum Preparo {
    /// Sem texto: só estilo.
    Vazia,
    Literal(Value),
    Formula {
        /// O padrão da coluna empacotada (`None`: não cabe nela).
        padrao: Option<Arc<str>>,
        /// `None` quando o padrão cabe na coluna empacotada: lá a árvore
        /// vem do modelo do padrão, e montá-la aqui seria só lixo.
        arvore: Option<Result<Node, String>>,
        volatil: bool,
    },
}

/// Uma célula lida do arquivo, pronta para [`Sheet::carregar_preparada`].
#[derive(Debug)]
pub struct CelulaPreparada {
    pub row: u32,
    pub col: u32,
    /// Índice do estilo no arquivo (o `s` do XLSX); quem grava traduz.
    pub estilo: usize,
    /// Onde está o texto da célula no `texto` do lote.
    pub faixa: (u32, u32),
    pub preparo: Preparo,
    /// Resultado da fórmula que o arquivo já traz (o `<v>` do XLSX salvo
    /// pelo Excel). Vale até algo de que ela depende mudar — como no Excel
    /// —, e a primeira tela não precisa recalcular nada.
    pub guardado: Option<Value>,
    /// O `round_trips` do literal já feito por quem preparou (em paralelo);
    /// `None`: quem grava faz. Era 30% da gravação de 13 milhões de células,
    /// que roda numa thread só (28/10/2026).
    pub ida_e_volta: Option<bool>,
}

impl CelulaPreparada {
    /// Faz aqui (fora da thread que grava) a conferência se o texto do
    /// literal se reconstrói pelo valor e o `estilo` (o mesmo que a gravação
    /// vai receber).
    pub fn com_ida_e_volta(mut self, raw: &str, estilo: Option<&EstiloParcial>) -> Self {
        if let Preparo::Literal(valor) = &self.preparo {
            self.ida_e_volta = Some(round_trips(raw, valor, estilo));
        }
        self
    }

    /// O texto da célula, de dentro do texto do lote.
    pub fn raw<'a>(&self, texto: &'a str) -> &'a str {
        &texto[self.faixa.0 as usize..self.faixa.1 as usize]
    }

    /// Não lê a aba: pode rodar em qualquer thread.
    pub fn preparar(row: u32, col: u32, raw: &str, faixa: (u32, u32), estilo: usize,
        guardado: Option<Value>, padroes: &mut Padroes) -> Self {
        let aparado = raw.trim();
        let preparo = if aparado.is_empty() {
            Preparo::Vazia
        } else if let Some(sem_igual) = aparado.strip_prefix('=') {
            let padrao = pattern_for(aparado, row as i32, col as i32).map(|p| {
                if let Some((partilhado, cabe)) = padroes.0.get(&p) {
                    return (partilhado.clone(), *cabe);
                }
                let cabe = FormulaTable::aceitaria(&p, aparado, pattern_check);
                let partilhado: Arc<str> = Arc::from(p.as_str());
                padroes.0.insert(p, (partilhado.clone(), cabe));
                (partilhado, cabe)
            });
            let cabe = padrao.as_ref().is_some_and(|(_, cabe)| *cabe);
            Preparo::Formula {
                arvore: (!cabe).then(|| parse_formula(sem_igual).map_err(|e| e.message)),
                padrao: padrao.map(|(p, _)| p),
                volatil: is_volatile(sem_igual),
            }
        } else {
            Preparo::Literal(crate::celulas::input::parse_literal(aparado).value)
        };
        CelulaPreparada { row, col, estilo, faixa, preparo, guardado, ida_e_volta: None }
    }
}

/// O que a gravação não guardou de uma célula, para soltar em outra thread.
#[derive(Debug, Default)]
pub struct Sobra {
    pub arvore: Option<Node>,
    pub padrao: Option<Arc<str>>,
    pub valor: Option<Value>,
}

impl Sheet {
    /// Grava uma célula preparada (`raw` é o texto dela, de
    /// [`CelulaPreparada::raw`]), com o estilo já registrado (`estilo_idx`,
    /// de [`Self::registrar_parcial`]). Se a célula já tiver algo (não
    /// deveria, numa aba recém-criada), vai pelo `apply` comum. Ao terminar
    /// o lote, chamar [`Self::terminar_carga`].
    ///
    /// O que sobra da célula não é liberado aqui: vai para `sobras`, e quem
    /// chama solta em outra thread (ver a nota do módulo).
    pub fn carregar_preparada(&mut self, c: CelulaPreparada, raw: &str, estilo_idx: Option<usize>,
        estilo: Option<&EstiloParcial>, sobras: &mut Vec<Sobra>) {
        let CelulaPreparada { row, col, preparo, guardado, ida_e_volta, .. } = c;
        if row >= MAX_ROWS || col >= MAX_COLS {
            return;
        }
        let k = key(row, col);
        if self.cells.contains_key(&k) || self.data.has_cell(row, col) || self.formulas.has(row, col)
            || self.styles.contains_key(&k) {
            self.apply(row, col, Some(Snap { raw: raw.to_string(), style: estilo.cloned() }), false);
            return;
        }
        match preparo {
            Preparo::Vazia => {
                if let Some(i) = estilo_idx {
                    self.styles.insert(k, i);
                    self.cresce_limites(row, col);
                }
            }
            Preparo::Literal(valor) => {
                self.cresce_limites(row, col);
                if let Value::Error(_) = valor {
                    // Erro digitado ("#N/A"): não cabe na coluna de dado.
                    self.cells.insert(k, CellData { raw: raw.to_string(), formula: None, value: valor, style_idx: estilo_idx });
                    return;
                }
                self.data.set_cell_value(row, col, &valor, &mut self.pool);
                if let Some(i) = estilo_idx {
                    self.styles.insert(k, i);
                }
                // Mesma regra do `apply`: guarda o texto quando a leitura
                // não o reconstruiria.
                if !ida_e_volta.unwrap_or_else(|| round_trips(raw, &valor, estilo))
                    || self.data.get_raw(row, col, &self.pool) != raw {
                    self.textos.insert(k, raw);
                }
                if matches!(valor, Value::Text(_)) {
                    sobras.push(Sobra { valor: Some(valor), ..Default::default() });
                }
            }
            Preparo::Formula { padrao, arvore, volatil } => {
                self.cresce_limites(row, col);
                // Volátil (`AGORA`) não aproveita o valor do arquivo: muda sozinho.
                let guardado = guardado.filter(|_| !volatil);
                if self.formulas.set_com_padrao(row, col, raw.trim(), padrao.as_deref(), pattern_check) {
                    if let Some(i) = estilo_idx {
                        self.styles.insert(k, i);
                    }
                    if let Some(v) = &guardado {
                        self.formulas.store(row, col, v, &mut self.pool);
                    }
                    sobras.push(Sobra { arvore: arvore.and_then(Result::ok), padrao, valor: guardado });
                    return;
                }
                // Não coube (teto de padrões): a árvore que não veio é
                // montada aqui.
                let arvore = arvore.unwrap_or_else(|| {
                    let aparado = raw.trim();
                    parse_formula(aparado.strip_prefix('=').unwrap_or(aparado)).map_err(|e| e.message)
                });
                let (formula, value) = match arvore {
                    Ok(node) => {
                        let nome = self.name.clone();
                        let is_self = move |s: Option<&str>| s.is_none_or(|s| super::mesmo_nome(s, &nome));
                        self.deps.link(k, &node, &is_self, &mut |_, _| {});
                        if le_de_fora(&node, &self.name) {
                            self.externas.insert(k);
                        }
                        if volatil {
                            self.deps.volatiles.insert(k);
                        }
                        (Some(node), Value::Empty)
                    }
                    Err(motivo) => (None, Value::Error(CellError::com_motivo(ErrorCode::Name, motivo))),
                };
                // Fórmula que não compilou fica com o erro, não com o valor do arquivo.
                let guardado = guardado.filter(|_| formula.is_some());
                sobras.push(Sobra { padrao, ..Default::default() });
                self.cells.insert(k, CellData { raw: raw.to_string(), formula, value, style_idx: estilo_idx });
                // Âncora de matriz recalcula ao abrir: o arquivo guarda só o
                // primeiro item, e a área derramada precisa ser montada.
                if let Some(v) = guardado.filter(|_| !self.deps.array_cells.contains(&k)) {
                    self.calc.get_mut().cache.insert(k, v);
                }
            }
        }
    }

    /// Fim da carga em lote: nada do que foi gravado tem valor calculado, e
    /// a grade redesenha tudo.
    pub fn terminar_carga(&mut self) {
        self.tocar();
        self.rastro.borrow_mut().desconhecer();
    }

    /// Carga do `.fshz`: uma coluna inteira de números, direto nos blocos.
    /// Era um `set_cell_valor` por célula — 6 milhões de chamadas na
    /// Planilha Pesada, cada uma acertando limites e procurando a coluna.
    pub fn carregar_coluna_numeros(&mut self, col: u32, values: &[f64], bitmap: &[u8]) {
        if col >= MAX_COLS {
            return;
        }
        let linhas = (values.len() as u32).min(MAX_ROWS);
        let Some((primeira, ultima)) = pontas_do_bitmap(bitmap, linhas) else { return };
        self.data.carregar_coluna_num(col, values, bitmap, linhas);
        self.cresce_limites(primeira, col);
        self.cresce_limites(ultima, col);
    }

    /// Carga do `.fshz`: uma coluna inteira de texto, com o dicionário local
    /// do arquivo traduzido para o pool da aba uma vez por texto distinto.
    pub fn carregar_coluna_textos(&mut self, col: u32, indices: &[i32], bitmap: &[u8], dict: &[String]) {
        if col >= MAX_COLS {
            return;
        }
        let linhas = (indices.len() as u32).min(MAX_ROWS);
        let Some((primeira, ultima)) = pontas_do_bitmap(bitmap, linhas) else { return };
        let ids_pool: Vec<i32> = dict.iter().map(|t| self.pool.get_or_create(t)).collect();
        let mut ids = vec![0i32; linhas as usize];
        let mut mascara = bitmap[..((linhas as usize) + 7) >> 3].to_vec();
        for (r, destino) in ids.iter_mut().enumerate() {
            if mascara[r >> 3] & (1 << (r & 7)) == 0 {
                continue;
            }
            match ids_pool.get(indices[r] as usize).copied() {
                // Texto vazio ou pool cheio: a célula fica sem nada, como no
                // `set_cell_value`.
                Some(id) if id >= 0 && !self.pool.get(id).is_empty() => *destino = id,
                _ => mascara[r >> 3] &= !(1 << (r & 7)),
            }
        }
        self.data.carregar_coluna_txt(col, &ids, &mascara, linhas);
        self.cresce_limites(primeira, col);
        self.cresce_limites(ultima, col);
    }
}

impl Sheet {
    /// Carga do CSV por colunas: um bloco de 65.536 linhas (`CHUNK_SIZE`) de
    /// números de uma coluna, entregue pronto (nada é copiado). A aba tem de
    /// estar vazia nesse bloco. Ao terminar, [`Self::terminar_carga`].
    pub fn carregar_bloco_numeros(&mut self, col: u32, idx: usize, valores: Box<[f64]>, mascara: Box<[u8]>) {
        if col >= MAX_COLS || (idx << CHUNK_SHIFT) as u32 >= MAX_ROWS {
            return;
        }
        if let Some((primeira, ultima)) = self.data.carregar_bloco_num(col, idx, valores, mascara) {
            self.cresce_limites(primeira, col);
            self.cresce_limites(ultima, col);
        }
    }

    /// Como [`Self::carregar_bloco_numeros`], para texto: `ids` aponta para
    /// `dict` (os textos distintos do bloco), que entra no pool da aba uma
    /// vez por texto. Texto vazio ou pool cheio deixam a célula sem nada,
    /// como no `set_cell_value`.
    pub fn carregar_bloco_textos(&mut self, col: u32, idx: usize, mut ids: Box<[i32]>, mut mascara: Box<[u8]>, dict: &[&str]) {
        if col >= MAX_COLS || (idx << CHUNK_SHIFT) as u32 >= MAX_ROWS {
            return;
        }
        let no_pool: Vec<i32> = dict.iter()
            .map(|t| if t.is_empty() { -1 } else { self.pool.get_or_create(t) }).collect();
        for (r, id) in ids.iter_mut().enumerate() {
            let bit = 1u8 << (r & 7);
            if mascara[r >> 3] & bit == 0 {
                continue;
            }
            match no_pool.get(*id as usize).copied() {
                Some(novo) if novo >= 0 => *id = novo,
                _ => mascara[r >> 3] &= !bit,
            }
        }
        if let Some((primeira, ultima)) = self.data.carregar_bloco_txt(col, idx, ids, mascara) {
            self.cresce_limites(primeira, col);
            self.cresce_limites(ultima, col);
        }
    }
}

/// A primeira e a última linha presentes na máscara, se houver alguma.
fn pontas_do_bitmap(bitmap: &[u8], linhas: u32) -> Option<(u32, u32)> {
    let ligado = |r: u32| bitmap.get((r >> 3) as usize).is_some_and(|b| b & (1 << (r & 7)) != 0);
    let primeira = (0..linhas).find(|&r| ligado(r))?;
    let ultima = (0..linhas).rev().find(|&r| ligado(r)).unwrap_or(primeira);
    Some((primeira, ultima))
}
