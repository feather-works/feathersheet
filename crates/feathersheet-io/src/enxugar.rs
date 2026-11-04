//! Enxuga um `.fshz` FSH2 **antes** de ele atravessar a ponte para a WebView.
//!
//! # ⚠ PROCESSO CRÍTICO — carregamento de arquivo
//!
//! Este arquivo faz parte do caminho de abertura de `.fshz`, que já derrubou a
//! máquina do usuário mais de uma vez (18,8 GB na WebView em 22/09/2026, com o
//! sistema perguntando se queria encerrar o processo). Antes de mexer:
//!
//!  1. **meça no aplicativo rodando**, não em teste sintético nem em `examples`
//!     — três tentativas anteriores foram revertidas por terem "melhorado"
//!     números que não eram os que travavam a máquina;
//!  2. o medidor é a janelinha de carregamento (`LoadingOverlay.medir`), que lê
//!     a **árvore inteira** de processos e o **pico** (`VmHWM`). RSS do
//!     processo do Tauri sozinho, ou lido depois da função, mente;
//!  3. cada cópia dos dados aqui vale centenas de MB. Conte quantas ficam vivas
//!     ao mesmo tempo antes de acrescentar uma.
//!
//!
//! # Por que isto existe
//!
//! Medido no aplicativo rodando, abrindo um `.fshz` de 73 MB com 10 milhões de
//! linhas (22/09/2026, com `LoadingOverlay.medir`, o primeiro medidor deste
//! projeto que enxerga o processo da WebView e não só o do Tauri):
//!
//! ```text
//! antes de ler                    541 MB
//! ler_arquivo (73 MB)             774 MB   (+233)
//! bytes na WebView              1.123 MB   (+349)
//! gunzip                        3.652 MB   (+2.529)   5,4 s
//! metadados (JSON)              2.784 MB   (-868, o coletor passou)
//! carregarV2                    7.731 MB   (+4.947)   7,2 s
//! ```
//!
//! Os dois saltos são o `gunzip` feito em JavaScript e o `carregarV2`, que
//! transforma **10.383.868 células avulsas** em objetos — 480 bytes por
//! célula. Destas, **9.999.881 (96,3%) são vazias**, todas na mesma coluna,
//! todas com o mesmo estilo: são o que sobrou de "formatei a coluna H como
//! moeda" numa época em que isso materializava uma célula por linha. Dez
//! milhões de células para guardar uma frase.
//!
//! O `JSON.parse` dos 325 MB de metadados, que era o suspeito óbvio, **não é**
//! o problema: a memória até cai nessa etapa. Quem apontou isso foi o usuário,
//! observando que a versão web abria o mesmo arquivo sem estourar — e a versão
//! web faz exatamente o mesmo `JSON.parse`.
//!
//! Então este módulo faz, do lado Rust, as duas coisas que a WebView não
//! deveria fazer: descomprime, e colapsa a coluna fantasma num `colStyles`.
//! O que sai tem **o mesmo formato** que o TypeScript já sabe ler
//! (`[u32 metaLen][metadados JSON][região Arrow]`), só que descomprimido e sem
//! as fantasmas — de propósito: a tentativa anterior (ponte PIO2, 21/09,
//! revertida por ter piorado o consumo) inventou um formato binário novo, e o
//! custo de conferir os dois lados foi maior que o ganho.
//!
//! # A regra do colapso, e por que ela é tão restrita
//!
//! Colapsar demais apaga formatação de verdade. Uma coluna só vira `colStyles`
//! quando **todas** estas valem:
//!
//! 1. a coluna **não tem coluna Arrow** — quem tem dado colunar tem estilo em
//!    `st`, e conferir isso exigiria ler a região Arrow, que é justamente o que
//!    não queremos tocar aqui;
//! 2. todas as vazias da coluna têm **o mesmo** índice de estilo;
//! 3. vazias + cheias **cobrem a altura inteira** da aba (faixa parcial não
//!    colapsa, senão pintaria linha que hoje está limpa);
//! 4. **toda célula com conteúdo da coluna já tem estilo próprio** (em `c.s` ou
//!    em `st`). Sem isto, uma célula que hoje não tem estilo nenhum passaria a
//!    herdar o da coluna — foi o caso encontrado no arquivo real, onde as 119
//!    células com conteúdo da coluna H têm estilo só no `st`;
//! 5. há pelo menos [`MINIMO_PARA_COLAPSAR`] vazias, para não trocar seis
//!    células por um registro e ainda mudar a semântica.
//!
//! Com a 4 valendo, o resultado na tela é idêntico: o `getStyle` funde
//! planilha -> coluna -> linha -> direto, e o estilo direto continua vencendo.
//!
//! Uma diferença assumida: depois do colapso, `getStyle` de uma linha **abaixo**
//! da altura da aba devolve o estilo da coluna, onde antes devolvia nada. É o
//! que a pessoa pediu ao formatar a coluna inteira.

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Colunas por linha na chave de célula: `k = linha * 16384 + coluna`. É a
/// mesma conta do `refs.ts`/`fshz-arrow.ts` — mudar aqui sem mudar lá troca a
/// planilha de lugar.
const COLUNAS: u64 = 16_384;

/// Abaixo disto não vale colapsar: o ganho é nenhum e o risco de mudar
/// semântica continua inteiro.
const MINIMO_PARA_COLAPSAR: usize = 1_024;

/// Uma célula avulsa, como o `fshz-arrow.ts` a grava.
#[derive(Deserialize, Serialize)]
struct Avulsa {
    k: u64,
    v: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    s: Option<u32>,
}

/// As avulsas de uma aba, já separadas na hora de ler.
///
/// A separação acontece **durante** a desserialização, e é o ponto do módulo:
/// as vazias nunca viram uma `Avulsa` guardada. Ficam como um par
/// `(chave, estilo)` de 12 bytes, contra os ~500 que um objeto com `String`
/// custaria — 10 milhões de vazias são 120 MB assim, e vários GB do outro
/// jeito.
#[derive(Default)]
struct Avulsas {
    cheias: Vec<Avulsa>,
    vazias: Vec<(u64, u32)>,
    /// Vazias sem estilo nenhum: não servem para nada e morrem aqui.
    descartadas: u64,
}

impl<'de> Deserialize<'de> for Avulsas {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Avulsas;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a lista de células avulsas")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Avulsas, A::Error> {
                let mut fora = Avulsas::default();
                if let Some(n) = seq.size_hint() {
                    fora.cheias.reserve(n.min(1 << 20));
                }
                while let Some(cel) = seq.next_element::<Avulsa>()? {
                    if !cel.v.is_empty() {
                        fora.cheias.push(cel);
                    } else if let Some(s) = cel.s {
                        fora.vazias.push((cel.k, s));
                    } else {
                        fora.descartadas += 1;
                    }
                }
                Ok(fora)
            }
        }
        d.deserialize_seq(V)
    }
}

/// Os metadados de uma aba.
///
/// Os campos são enumerados um a um, e `deny_unknown_fields` é de propósito:
/// como esta estrutura é reserializada, um campo novo em `AbaMeta`
/// (`fshz-arrow.ts`) que não estivesse aqui seria **perdido em silêncio**.
/// Assim ele vira um erro barulhento, e quem chama cai no leitor antigo, que
/// não perde nada. Se você acrescentou um campo lá, acrescente aqui.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Aba {
    n: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tc: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    w: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    h: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    g: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pr: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    r: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tb: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oq: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dn: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mc: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    im: Option<Value>,
    /// As respostas das tarefas assíncronas. Só atravessa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pd: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    va: Option<Value>,
    /// Botões de gatilho de macro. Só atravessa; o enxugador não mexe nele.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bt: Option<Value>,
    /// Tinta da aba Desenhar. Só atravessa, como os botões.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tn: Option<Value>,
    /// Objetos soltos sobre a grade (formas). Só atravessa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ob: Option<Value>,
    /// Minigráficos da aba. Só atravessa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mg: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ce: Option<Value>,
    /// As avulsas. Entram separadas em cheias/vazias e saem como `Vec<Avulsa>`
    /// pelo `serialize_with`, para o TypeScript continuar lendo o mesmo campo.
    #[serde(default, skip_serializing)]
    c: Avulsas,
    /// O que sobra das avulsas depois do colapso — é este que vai para o JSON,
    /// com o nome `c`.
    #[serde(skip_deserializing, rename = "c")]
    c_saida: Vec<Avulsa>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    st: Option<Vec<(u64, u32)>>,
    /// Estilo por coluna: `[coluna, índice na tabela de estilos]`. Nasce aqui,
    /// no colapso, e o `carregarV2` o aplica em `sheet.colStyles`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cs: Vec<(u64, u32)>,
    /// Estilo de linha inteira. Só atravessa; o enxugador não mexe nele.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rs: Option<Value>,
    /// Estilo da aba inteira. Idem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ss: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pat: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    len: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cols: Option<Vec<ColunaMeta>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ar: Option<Value>,
}

/// Só o que este módulo precisa saber de uma coluna Arrow: qual é.
#[derive(Deserialize, Serialize)]
struct ColunaMeta {
    f: String,
    c: u64,
    k: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    d: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    e: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fv: Option<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Livro {
    v: u32,
    a: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    names: Option<Value>,
    t: Vec<Value>,
    s: Vec<Aba>,
}

/// Quanto o enxugamento economizou, para quem quiser registrar em log.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Resumo {
    pub avulsas_antes: u64,
    pub avulsas_depois: u64,
    pub colunas_colapsadas: u64,
    pub vazias_descartadas: u64,
}

/// Decide o colapso de uma aba e devolve as avulsas que sobram.
///
/// Ver a regra, e o porquê de cada item, no cabeçalho do módulo.
fn colapsar(aba: &mut Aba) -> u64 {
    let Some(altura) = aba.len else { return 0 };
    if altura == 0 {
        return 0;
    }
    let avulsas = std::mem::take(&mut aba.c);

    // Colunas que têm dado colunar: ficam de fora (item 1).
    let com_arrow: HashSet<u64> = aba
        .cols
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|c| c.c)
        .collect();

    // Quem já tem estilo direto, por chave (item 4).
    let mut com_estilo: HashSet<u64> = aba
        .st
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|(k, _)| *k)
        .collect();
    for cel in &avulsas.cheias {
        if cel.s.is_some() {
            com_estilo.insert(cel.k);
        }
    }

    // Por coluna: o estilo das vazias (None = mais de um, não colapsa),
    // quantas vazias, quantas cheias, e se toda cheia tem estilo próprio.
    struct PorColuna {
        estilo: Option<u32>,
        um_estilo_so: bool,
        vazias: u64,
        cheias: u64,
        toda_cheia_tem_estilo: bool,
    }
    let mut mapa: HashMap<u64, PorColuna> = HashMap::new();
    let novo = || PorColuna {
        estilo: None,
        um_estilo_so: true,
        vazias: 0,
        cheias: 0,
        toda_cheia_tem_estilo: true,
    };
    for (k, s) in &avulsas.vazias {
        let e = mapa.entry(k % COLUNAS).or_insert_with(novo);
        match e.estilo {
            None if e.vazias == 0 => e.estilo = Some(*s),
            Some(ja) if ja != *s => e.um_estilo_so = false,
            _ => {}
        }
        e.vazias += 1;
    }
    for cel in &avulsas.cheias {
        let e = mapa.entry(cel.k % COLUNAS).or_insert_with(novo);
        e.cheias += 1;
        if !com_estilo.contains(&cel.k) {
            e.toda_cheia_tem_estilo = false;
        }
    }

    let colapsar: HashMap<u64, u32> = mapa
        .iter()
        .filter(|(col, e)| {
            !com_arrow.contains(col)
                && e.um_estilo_so
                && e.vazias as usize >= MINIMO_PARA_COLAPSAR
                && e.vazias + e.cheias >= altura
                && e.toda_cheia_tem_estilo
        })
        .filter_map(|(col, e)| e.estilo.map(|s| (*col, s)))
        .collect();

    // As cheias sempre ficam; as vazias só ficam se a coluna não colapsou.
    let mut saida = avulsas.cheias;
    for (k, s) in avulsas.vazias {
        if !colapsar.contains_key(&(k % COLUNAS)) {
            saida.push(Avulsa { k, v: String::new(), s: Some(s) });
        }
    }
    aba.c_saida = saida;

    let mut cs: Vec<(u64, u32)> = colapsar.into_iter().collect();
    cs.sort_unstable();
    let quantas = cs.len() as u64;
    aba.cs = cs;
    quantas
}

/// O corpo já descomprimido: `[u32 metaLen][metadados JSON][região Arrow]`.
///
/// Devolve o mesmo formato, com os metadados enxutos. A região Arrow é copiada
/// byte a byte e **as fatias `ar` continuam válidas** porque nada nela se move.
pub fn enxugar_corpo(mut corpo: Vec<u8>) -> Result<(Vec<u8>, Resumo), String> {
    if corpo.len() < 4 {
        return Err("corpo curto demais para ter o tamanho dos metadados".into());
    }
    let meta_len = u32::from_le_bytes([corpo[0], corpo[1], corpo[2], corpo[3]]) as usize;
    let fim = 4 + meta_len;
    if corpo.len() < fim {
        return Err(format!("metadados dizem ter {meta_len} bytes, o corpo não tem"));
    }

    let mut livro: Livro = serde_json::from_slice(&corpo[4..fim])
        .map_err(|e| format!("metadados FSH2 não reconhecidos: {e}"))?;

    let mut resumo = Resumo::default();
    for aba in &mut livro.s {
        resumo.avulsas_antes += aba.c.cheias.len() as u64 + aba.c.vazias.len() as u64;
        resumo.vazias_descartadas += aba.c.descartadas;
        resumo.colunas_colapsadas += colapsar(aba);
        resumo.avulsas_depois += aba.c_saida.len() as u64;
    }

    let meta = serde_json::to_vec(&livro).map_err(|e| format!("não consegui reescrever os metadados: {e}"))?;
    drop(livro);

    // A montagem é FEITA POR CIMA do próprio `corpo`, não num buffer novo.
    //
    // A região Arrow tem 611 MB no arquivo real. Copiá-la para um `Vec` novo
    // significava as duas vivas ao mesmo tempo — 600 MB de pico à toa, no
    // exato momento em que o consumo já está no alto. Como os metadados só
    // encolhem (325 MB viram alguns KB), o novo cabeçalho sempre cabe onde o
    // antigo estava, e basta arrastar a região Arrow para a esquerda e cortar
    // o resto. `copy_within` é um `memmove`: aceita a sobreposição.
    let inicio_arrow = 4 + meta.len();
    debug_assert!(inicio_arrow <= fim, "os metadados enxutos não podem crescer");
    corpo[..4].copy_from_slice(&(meta.len() as u32).to_le_bytes());
    corpo[4..inicio_arrow].copy_from_slice(&meta);
    drop(meta);
    corpo.copy_within(fim.., inicio_arrow);
    let tamanho = inicio_arrow + (corpo.len() - fim);
    corpo.truncate(tamanho);
    corpo.shrink_to_fit();
    Ok((corpo, resumo))
}

/// O arquivo inteiro, como está no disco: `FSH2` + gzip.
///
/// Descomprimir aqui é metade do ganho — em JavaScript essa etapa custava
/// +2.529 MB e 5,4 s, medidos no aplicativo (ver o cabeçalho). Um `.fshz` que
/// não seja FSH2 devolve erro, e quem chama cai no leitor antigo.
pub fn ler_e_enxugar(bytes: &[u8]) -> Result<(Vec<u8>, Resumo), String> {
    use std::io::Read;

    if bytes.len() < 4 || &bytes[..4] != b"FSH2" {
        return Err("não é um FSH2".into());
    }
    let mut corpo = Vec::new();
    flate2::read::GzDecoder::new(&bytes[4..])
        .read_to_end(&mut corpo)
        .map_err(|e| format!("não consegui descomprimir o FSH2: {e}"))?;
    enxugar_corpo(corpo)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Monta um corpo `[metaLen][meta][arrow]` a partir do JSON dos metadados.
    fn corpo(meta: &str, arrow: &[u8]) -> Vec<u8> {
        let mut v = (meta.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(meta.as_bytes());
        v.extend_from_slice(arrow);
        v
    }

    /// Lê os metadados de volta de um corpo enxuto.
    fn meta_de(saida: &[u8]) -> Value {
        let n = u32::from_le_bytes([saida[0], saida[1], saida[2], saida[3]]) as usize;
        serde_json::from_slice(&saida[4..4 + n]).unwrap()
    }

    /// Uma coluna inteira vazia com o mesmo estilo vira um registro só.
    #[test]
    fn coluna_fantasma_vira_col_style() {
        let altura = 5000u64;
        let mut celulas = Vec::new();
        for r in 0..altura {
            celulas.push(format!(r#"{{"k":{},"v":"","s":1}}"#, r * COLUNAS + 7));
        }
        let meta = format!(
            r#"{{"v":2,"a":0,"t":[{{}},{{"format":"currency"}}],"s":[{{"n":"P","len":{altura},"c":[{}]}}]}}"#,
            celulas.join(",")
        );
        let (saida, resumo) = enxugar_corpo(corpo(&meta, b"ARROW")).unwrap();

        assert_eq!(resumo.avulsas_antes, altura);
        assert_eq!(resumo.avulsas_depois, 0, "nenhuma avulsa deveria sobrar");
        assert_eq!(resumo.colunas_colapsadas, 1);
        let m = meta_de(&saida);
        assert_eq!(m["s"][0]["cs"], serde_json::json!([[7, 1]]));
        assert_eq!(m["s"][0]["c"], serde_json::json!([]));
        // A região Arrow tem de sair intacta: as fatias `ar` apontam para ela.
        assert_eq!(&saida[saida.len() - 5..], b"ARROW");
    }

    /// Faixa parcial NÃO colapsa: pintaria linha que hoje está limpa.
    #[test]
    fn faixa_parcial_nao_colapsa() {
        let mut celulas = Vec::new();
        for r in 0..2000u64 {
            celulas.push(format!(r#"{{"k":{},"v":"","s":1}}"#, r * COLUNAS + 7));
        }
        let meta = format!(
            r#"{{"v":2,"a":0,"t":[{{}},{{"format":"currency"}}],"s":[{{"n":"P","len":9000,"c":[{}]}}]}}"#,
            celulas.join(",")
        );
        let (_, resumo) = enxugar_corpo(corpo(&meta, b"")).unwrap();
        assert_eq!(resumo.colunas_colapsadas, 0);
        assert_eq!(resumo.avulsas_depois, 2000, "as vazias precisam continuar lá");
    }

    /// Célula com conteúdo e SEM estilo próprio impede o colapso: senão ela
    /// passaria a herdar o estilo da coluna, e mudaria de aparência.
    #[test]
    fn cheia_sem_estilo_impede_o_colapso() {
        let altura = 3000u64;
        let mut celulas = vec![format!(r#"{{"k":{},"v":"oi"}}"#, 7)];
        for r in 1..altura {
            celulas.push(format!(r#"{{"k":{},"v":"","s":1}}"#, r * COLUNAS + 7));
        }
        let meta = format!(
            r#"{{"v":2,"a":0,"t":[{{}},{{"format":"currency"}}],"s":[{{"n":"P","len":{altura},"c":[{}]}}]}}"#,
            celulas.join(",")
        );
        let (_, resumo) = enxugar_corpo(corpo(&meta, b"")).unwrap();
        assert_eq!(resumo.colunas_colapsadas, 0);
    }

    /// A mesma coluna, agora com a cheia tendo estilo no `st` — é a forma do
    /// arquivo real (as 119 células com conteúdo da coluna H). Colapsa.
    #[test]
    fn cheia_com_estilo_no_st_permite_o_colapso() {
        let altura = 3000u64;
        let mut celulas = vec![format!(r#"{{"k":{},"v":"oi"}}"#, 7)];
        for r in 1..altura {
            celulas.push(format!(r#"{{"k":{},"v":"","s":1}}"#, r * COLUNAS + 7));
        }
        let meta = format!(
            r#"{{"v":2,"a":0,"t":[{{}},{{"format":"currency"}},{{"format":"auto"}}],"s":[{{"n":"P","len":{altura},"st":[[7,2]],"c":[{}]}}]}}"#,
            celulas.join(",")
        );
        let (saida, resumo) = enxugar_corpo(corpo(&meta, b"")).unwrap();
        assert_eq!(resumo.colunas_colapsadas, 1);
        assert_eq!(resumo.avulsas_depois, 1, "a célula com conteúdo fica");
        let m = meta_de(&saida);
        assert_eq!(m["s"][0]["st"], serde_json::json!([[7, 2]]), "o `st` precisa sobreviver");
    }

    /// Coluna com dado colunar não colapsa: o estilo das linhas com dado mora
    /// na região Arrow, que este módulo não lê.
    #[test]
    fn coluna_com_arrow_nao_colapsa() {
        let altura = 3000u64;
        let mut celulas = Vec::new();
        for r in 0..altura {
            celulas.push(format!(r#"{{"k":{},"v":"","s":1}}"#, r * COLUNAS + 7));
        }
        let meta = format!(
            r#"{{"v":2,"a":0,"t":[{{}},{{"format":"currency"}}],"s":[{{"n":"P","len":{altura},"cols":[{{"f":"c7","c":7,"k":"num"}}],"c":[{}]}}]}}"#,
            celulas.join(",")
        );
        let (_, resumo) = enxugar_corpo(corpo(&meta, b"")).unwrap();
        assert_eq!(resumo.colunas_colapsadas, 0);
    }

    /// Vazia sem estilo nenhum não serve para nada e não volta.
    #[test]
    fn vazia_sem_estilo_morre() {
        let meta = r#"{"v":2,"a":0,"t":[],"s":[{"n":"P","len":10,"c":[{"k":0,"v":""},{"k":1,"v":"x"}]}]}"#;
        let (saida, resumo) = enxugar_corpo(corpo(meta, b"")).unwrap();
        assert_eq!(resumo.vazias_descartadas, 1);
        assert_eq!(resumo.avulsas_depois, 1);
        assert_eq!(meta_de(&saida)["s"][0]["c"][0]["v"], "x");
    }

    /// Campo desconhecido é erro barulhento, não perda silenciosa: os
    /// metadados são reserializados, então um campo novo em `AbaMeta` que não
    /// estivesse aqui sumiria do arquivo carregado.
    #[test]
    fn campo_novo_falha_alto() {
        let meta = r#"{"v":2,"a":0,"t":[],"s":[{"n":"P","len":10,"c":[],"zz":1}]}"#;
        let erro = enxugar_corpo(corpo(meta, b"")).unwrap_err();
        assert!(erro.contains("não reconhecidos"), "erro foi: {erro}");
    }

    /// Tudo que não é avulsa precisa atravessar sem perder nada.
    #[test]
    fn o_resto_dos_metadados_sobrevive() {
        let meta = r##"{"v":2,"a":1,"names":[["x","A1"]],"t":[{"bold":true}],"s":[{"n":"Aba","tc":"#f00","w":[[0,120]],"h":[[1,30]],"g":[{"t":"bar"}],"r":[{"op":"gt"}],"tb":[1],"oq":[2],"dn":[3],"bt":[{"id":"b1"}],"tn":{"v":1,"a":{"1":[0,0]},"t":"FTINTA 1\\nT 1 c 000000FF 2 1 s 0,0 5,5\\n"},"ob":{"v":1,"objetos":[{"id":1,"forma":"rect","de":{"linha":0,"coluna":0},"ate":{"linha":2,"coluna":1}}]},"ce":[{"nome":"Alta"}],"mg":[{"itens":[]}],"pat":["=A1+1"],"len":9,"cols":[{"f":"c0","c":0,"k":"fx","d":["a"],"e":[],"fv":"v0"}],"ar":[0,4],"st":[[5,0]],"c":[]}]}"##;
        let (saida, _) = enxugar_corpo(corpo(meta, b"ABCD")).unwrap();
        let m = meta_de(&saida);
        let esperado: Value = serde_json::from_str(meta).unwrap();
        for campo in ["v", "a", "names", "t"] {
            assert_eq!(m[campo], esperado[campo], "perdeu {campo}");
        }
        for campo in ["n", "tc", "w", "h", "g", "r", "tb", "oq", "dn", "bt", "tn", "ob", "ce", "mg", "pat", "len", "cols", "ar", "st"] {
            assert_eq!(m["s"][0][campo], esperado["s"][0][campo], "perdeu {campo}");
        }
    }

    /// O caminho inteiro, a partir de um arquivo gzipado de verdade.
    #[test]
    fn ida_e_volta_pelo_gzip() {
        use std::io::Write;
        let meta = r#"{"v":2,"a":0,"t":[],"s":[{"n":"P","len":4,"c":[{"k":0,"v":"x"}]}]}"#;
        let cru = corpo(meta, b"ARROW");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&cru).unwrap();
        let mut arquivo = b"FSH2".to_vec();
        arquivo.extend_from_slice(&gz.finish().unwrap());

        let (saida, resumo) = ler_e_enxugar(&arquivo).unwrap();
        assert_eq!(resumo.avulsas_depois, 1);
        assert_eq!(&saida[saida.len() - 5..], b"ARROW");
        assert!(ler_e_enxugar(b"XXX1qualquercoisa").is_err(), "assinatura errada não é deste leitor");
    }
}
