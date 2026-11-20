//! O quadro — o retângulo visível da planilha, em binário.
//!
//! É o caminho quente da ponte descrita em `docs/ARQUITETURA.md`: acontece a cada
//! rolagem, a cada tecla, a cada redesenho. JSON aqui seria um erro — milhares
//! de objetos criados e jogados fora sessenta vezes por segundo, do lado que
//! justamente não pode carregar peso.
//!
//! **Só o que está na tela entra.** Um quadro típico é de umas oitocentas
//! células; a planilha por trás pode ter dez milhões. É essa diferença que faz
//! o TypeScript deixar de guardar a planilha: ele tem o quadro de agora e joga
//! fora ao rolar.
//!
//! ## O formato
//!
//! Tudo em *little-endian*, que é o que `DataView` e as `TypedArray` do
//! JavaScript leem sem conversão em qualquer máquina que rode isto.
//!
//! ```text
//! 0   "QDR1"                 4 bytes
//! 4   versão do conteúdo     u32
//! ```
//!
//! Se a versão que o TypeScript já tem ainda vale, a resposta **para aqui**:
//! oito bytes dizendo "nada mudou". É o caso comum — a pessoa mexeu o mouse,
//! rolou de volta para onde já estava, a janela repintou. Ver
//! [`FRAME_SEM_MUDANCA`].
//!
//! Senão, continua:
//!
//! ```text
//! 8   r0, c0, r1, c1         u32 x4   o retângulo que este quadro descreve
//! 24  quantidade de células  u32
//! 28  quantidade de estilos  u32
//! 32  linha[]                u32 x n
//!     coluna[]               u32 x n
//!     bandeiras[]            u8  x n   (preenchido até múltiplo de 4)
//!     estilo[]               u32 x n   (índice no dicionário; SEM_ESTILO = nenhum)
//!     início do texto[]      u32 x n   (deslocamento dentro do bloco de texto)
//!     tamanho do texto[]     u32 x n   (em BYTES, não em caracteres)
//!     estilos[]              u32 x e   o dicionário citado por este quadro
//!     texto                  bytes UTF-8, tudo emendado
//! ```
//!
//! ## Dois detalhes que parecem manha e não são
//!
//! **As bandeiras são preenchidas até um múltiplo de 4.** Sem isso, com um
//! número ímpar de células os `u32` seguintes ficariam desalinhados, e o
//! JavaScript não poderia montar `new Uint32Array(buffer, offset, n)` por
//! cima deles — teria de copiar tudo, que é exatamente o custo que este
//! formato existe para evitar.
//!
//! **O tamanho do texto é em bytes.** "São Paulo" tem 9 caracteres e 10
//! bytes; quem fatiar o bloco por caractere corta no lugar errado. O
//! `TextDecoder` do lado de lá trabalha em bytes, então os dois concordam.

use crate::planilha::sheet::Sheet;
use crate::celulas::style::Style;
use crate::celulas::values::Value;

/// A assinatura no começo de todo quadro.
pub const MAGIC: [u8; 4] = *b"QDR1";

/// O tamanho da resposta "nada mudou": só a assinatura e a versão.
pub const FRAME_SEM_MUDANCA: usize = 8;

/// Onde começam as colunas paralelas, depois do cabeçalho.
const INICIO_DAS_COLUNAS: usize = 32;

/// Marca de "esta célula não tem estilo nenhum".
pub const SEM_ESTILO: u32 = u32::MAX;

/// A célula guarda um erro: o desenho pinta em vermelho sem olhar o texto.
pub const BANDEIRA_ERRO: u8 = 1;
/// A célula é número: alinha à direita.
pub const BANDEIRA_NUMERO: u8 = 2;
/// A célula é fórmula: importa para o "mostrar fórmulas" e para o editor.
pub const BANDEIRA_FORMULA: u8 = 4;
/// A célula é lógica (VERDADEIRO/FALSO): centraliza.
pub const BANDEIRA_LOGICO: u8 = 8;

/// O texto pronto para desenhar de um valor, e as bandeiras dele.
///
/// Quem formata é o Rust — é a regra do `docs/ARQUITETURA.md`, e ela apaga uma
/// classe inteira de bug (a mesma célula com dois formatadores diferentes).
///
/// O número sai pelo [`crate::celulas::style::format_value`] (o formato personalizado manda), com o estilo da célula:
/// moeda, porcentagem, data, duração, científico — tudo já resolvido aqui. A
/// fidelidade ao `style.ts` foi conferida em 1.344 casos, sem divergência.
///
/// ⚠ O que o quadro **ainda** não leva é o resto do estilo — negrito, cor,
/// fundo, alinhamento, bordas. Hoje vai só o índice. Esses atributos entram
/// quando a grade começar a desenhar do quadro, para o formato deles nascer
/// do que o desenho realmente pede, em vez de ser adivinhado agora.
fn texto_e_bandeiras(valor: &Value, eh_formula: bool, estilo: Option<&Style>) -> (String, u8) {
    let mut bandeiras = if eh_formula { BANDEIRA_FORMULA } else { 0 };
    let texto = match valor {
        Value::Empty => String::new(),
        Value::Number(n) => {
            bandeiras |= BANDEIRA_NUMERO;
            // O formato personalizado manda, como na grade.
            crate::celulas::style::format_value(&Value::Number(*n), estilo)
        }
        Value::Text(t) => t.clone(),
        Value::Bool(b) => {
            bandeiras |= BANDEIRA_LOGICO;
            if *b { "VERDADEIRO".to_string() } else { "FALSO".to_string() }
        }
        Value::Error(e) => {
            bandeiras |= BANDEIRA_ERRO;
            e.code.as_str().to_string()
        }
    };
    (texto, bandeiras)
}

/// Uma célula já resolvida, pronta para entrar no quadro.
struct Entrada {
    row: u32,
    col: u32,
    bandeiras: u8,
    estilo: u32,
    texto: String,
}

/// Monta o quadro do retângulo pedido.
///
/// `versao_conhecida` é a que o TypeScript já tem na mão. Quando ela bate com
/// a `versao` de agora, devolve os oito bytes de "nada mudou" — nenhuma
/// célula é lida, nenhum byte de texto é copiado.
///
/// O retângulo é fechado dos dois lados (`r0..=r1`), como o resto do motor.
pub fn build_frame(
    sheet: &Sheet,
    versao: u32,
    r0: u32,
    c0: u32,
    r1: u32,
    c1: u32,
    versao_conhecida: Option<u32>,
) -> Vec<u8> {
    let mut saida = Vec::new();
    saida.extend_from_slice(&MAGIC);
    saida.extend_from_slice(&versao.to_le_bytes());

    if versao_conhecida == Some(versao) {
        return saida;
    }

    // O retângulo pedido pode passar do que a planilha ocupa; pedir célula
    // fora do ocupado é barato (devolve vazio), mas não é de graça, então o
    // laço se contém no que existe. Uma tela rolada para o vazio devolve um
    // quadro sem célula nenhuma, que é o certo.
    let usado = sheet.used_rect();
    let lr0 = r0.max(usado.r0);
    let lc0 = c0.max(usado.c0);
    let lr1 = r1.min(usado.r1);
    let lc1 = c1.min(usado.c1);

    let mut entradas: Vec<Entrada> = Vec::new();
    let mut estilos: Vec<u32> = Vec::new();

    if lr0 <= lr1 && lc0 <= lc1 {
        for row in lr0..=lr1 {
            for col in lc0..=lc1 {
                let valor = sheet.get_value(row, col);
                let bruto = sheet.get_raw(row, col);
                let eh_formula = bruto.starts_with('=');
                let idx = sheet.get_style_idx(row, col);
                let estilo = idx.and_then(|i| sheet.style_table.get(i));
                let (texto, bandeiras) = texto_e_bandeiras(&valor, eh_formula, estilo);
                // Só célula com conteúdo entra — menos o caso da célula vazia
                // que tem estilo (um fundo pintado, uma borda), que o desenho
                // precisa conhecer para pintar.
                if texto.is_empty() && idx.is_none() {
                    continue;
                }
                let estilo = match idx {
                    None => SEM_ESTILO,
                    Some(i) => {
                        let i = i as u32;
                        match estilos.iter().position(|e| *e == i) {
                            Some(p) => p as u32,
                            None => {
                                estilos.push(i);
                                (estilos.len() - 1) as u32
                            }
                        }
                    }
                };
                entradas.push(Entrada { row, col, bandeiras, estilo, texto });
            }
        }
    }

    let n = entradas.len();
    let e = estilos.len();
    saida.extend_from_slice(&r0.to_le_bytes());
    saida.extend_from_slice(&c0.to_le_bytes());
    saida.extend_from_slice(&r1.to_le_bytes());
    saida.extend_from_slice(&c1.to_le_bytes());
    saida.extend_from_slice(&(n as u32).to_le_bytes());
    saida.extend_from_slice(&(e as u32).to_le_bytes());
    debug_assert_eq!(saida.len(), INICIO_DAS_COLUNAS);

    // O bloco de texto é montado junto, para o deslocamento de cada célula já
    // sair certo na primeira passada.
    let mut texto = Vec::<u8>::new();
    let mut inicios = Vec::with_capacity(n);
    let mut tamanhos = Vec::with_capacity(n);
    for entrada in &entradas {
        let bytes = entrada.texto.as_bytes();
        inicios.push(texto.len() as u32);
        tamanhos.push(bytes.len() as u32);
        texto.extend_from_slice(bytes);
    }

    for entrada in &entradas {
        saida.extend_from_slice(&entrada.row.to_le_bytes());
    }
    for entrada in &entradas {
        saida.extend_from_slice(&entrada.col.to_le_bytes());
    }
    for entrada in &entradas {
        saida.push(entrada.bandeiras);
    }
    // O enchimento que mantém os `u32` seguintes alinhados. Ver a nota do topo.
    while saida.len() % 4 != 0 {
        saida.push(0);
    }
    for entrada in &entradas {
        saida.extend_from_slice(&entrada.estilo.to_le_bytes());
    }
    for inicio in &inicios {
        saida.extend_from_slice(&inicio.to_le_bytes());
    }
    for tamanho in &tamanhos {
        saida.extend_from_slice(&tamanho.to_le_bytes());
    }
    for estilo in &estilos {
        saida.extend_from_slice(&estilo.to_le_bytes());
    }
    saida.extend_from_slice(&texto);
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lê um `u32` do quadro, no deslocamento pedido.
    fn u32_em(bytes: &[u8], pos: usize) -> u32 {
        u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
    }

    /// Desmonta o quadro do jeito que o TypeScript vai desmontar.
    struct Lido {
        n: usize,
        rows: Vec<u32>,
        cols: Vec<u32>,
        bandeiras: Vec<u8>,
        textos: Vec<String>,
        estilos: Vec<u32>,
    }

    fn ler(bytes: &[u8]) -> Lido {
        assert_eq!(&bytes[0..4], &MAGIC);
        let n = u32_em(bytes, 24) as usize;
        let e = u32_em(bytes, 28) as usize;
        let mut p = INICIO_DAS_COLUNAS;
        let rows: Vec<u32> = (0..n).map(|i| u32_em(bytes, p + i * 4)).collect();
        p += n * 4;
        let cols: Vec<u32> = (0..n).map(|i| u32_em(bytes, p + i * 4)).collect();
        p += n * 4;
        let bandeiras: Vec<u8> = bytes[p..p + n].to_vec();
        p += n;
        while p % 4 != 0 {
            p += 1;
        }
        let estilo_idx: Vec<u32> = (0..n).map(|i| u32_em(bytes, p + i * 4)).collect();
        p += n * 4;
        let inicios: Vec<u32> = (0..n).map(|i| u32_em(bytes, p + i * 4)).collect();
        p += n * 4;
        let tamanhos: Vec<u32> = (0..n).map(|i| u32_em(bytes, p + i * 4)).collect();
        p += n * 4;
        let dicionario: Vec<u32> = (0..e).map(|i| u32_em(bytes, p + i * 4)).collect();
        p += e * 4;
        let textos = (0..n)
            .map(|i| {
                let ini = p + inicios[i] as usize;
                let fim = ini + tamanhos[i] as usize;
                String::from_utf8(bytes[ini..fim].to_vec()).unwrap()
            })
            .collect();
        let estilos = estilo_idx
            .iter()
            .map(|i| if *i == SEM_ESTILO { SEM_ESTILO } else { dicionario[*i as usize] })
            .collect();
        Lido { n, rows, cols, bandeiras, textos, estilos }
    }

    fn planilha() -> Sheet {
        let mut s = Sheet::new("Planilha1");
        s.set_cell_raw(0, 0, "Produto".into(), None);
        s.set_cell_raw(0, 1, "Preço".into(), None);
        s.set_cell_raw(1, 0, "Teclado".into(), None);
        s.set_cell_raw(1, 1, "199,9".into(), None);
        s.set_cell_raw(2, 0, "Monitor".into(), None);
        s.set_cell_raw(2, 1, "1250".into(), None);
        s.set_cell_raw(3, 1, "=B2+B3".into(), None);
        s
    }

    #[test]
    fn o_quadro_leva_so_o_retangulo_pedido() {
        let s = planilha();
        let q = ler(&build_frame(&s, 1, 0, 0, 1, 1, None));
        assert_eq!(q.n, 4, "o retângulo 2x2 tem quatro células preenchidas");
        assert_eq!(q.rows, vec![0, 0, 1, 1]);
        assert_eq!(q.cols, vec![0, 1, 0, 1]);
        assert_eq!(q.textos, vec!["Produto", "Preço", "Teclado", "199,9"]);
    }

    #[test]
    fn celula_vazia_nao_entra() {
        let s = planilha();
        // A linha 4 (índice 3) só tem a coluna B preenchida.
        let q = ler(&build_frame(&s, 1, 3, 0, 3, 1, None));
        assert_eq!(q.n, 1, "a célula vazia da coluna A não podia entrar");
        assert_eq!(q.cols, vec![1]);
    }

    #[test]
    fn a_formula_chega_calculada_e_marcada() {
        let s = planilha();
        let q = ler(&build_frame(&s, 1, 3, 1, 3, 1, None));
        assert_eq!(q.textos, vec!["1.449,9"], "o quadro leva o VALOR já formatado, não o texto da fórmula");
        assert_eq!(q.bandeiras[0] & BANDEIRA_FORMULA, BANDEIRA_FORMULA);
        assert_eq!(q.bandeiras[0] & BANDEIRA_NUMERO, BANDEIRA_NUMERO);
    }

    #[test]
    fn numero_e_texto_se_distinguem_pela_bandeira() {
        let s = planilha();
        let q = ler(&build_frame(&s, 1, 1, 0, 1, 1, None));
        assert_eq!(q.textos, vec!["Teclado", "199,9"]);
        assert_eq!(q.bandeiras[0] & BANDEIRA_NUMERO, 0, "texto não é número");
        assert_eq!(q.bandeiras[1] & BANDEIRA_NUMERO, BANDEIRA_NUMERO);
    }

    #[test]
    fn o_erro_vem_marcado() {
        let mut s = Sheet::new("Planilha1");
        s.set_cell_raw(0, 0, "=1/0".into(), None);
        let q = ler(&build_frame(&s, 1, 0, 0, 0, 0, None));
        assert_eq!(q.textos, vec!["#DIV/0!"]);
        assert_eq!(q.bandeiras[0] & BANDEIRA_ERRO, BANDEIRA_ERRO);
    }

    #[test]
    fn versao_que_ainda_vale_devolve_oito_bytes() {
        let s = planilha();
        let bytes = build_frame(&s, 7, 0, 0, 100, 100, Some(7));
        assert_eq!(bytes.len(), FRAME_SEM_MUDANCA);
        assert_eq!(&bytes[0..4], &MAGIC);
        assert_eq!(u32_em(&bytes, 4), 7);
    }

    #[test]
    fn versao_diferente_devolve_o_quadro_inteiro() {
        let s = planilha();
        let bytes = build_frame(&s, 8, 0, 0, 100, 100, Some(7));
        assert!(bytes.len() > FRAME_SEM_MUDANCA);
        assert_eq!(ler(&bytes).n, 7);
    }

    #[test]
    fn o_estilo_vira_dicionario_sem_repetir() {
        let mut s = Sheet::new("Planilha1");
        s.set_cell_raw(0, 0, "a".into(), Some(4));
        s.set_cell_raw(0, 1, "b".into(), Some(4));
        s.set_cell_raw(0, 2, "c".into(), Some(9));
        s.set_cell_raw(0, 3, "d".into(), None);
        let bytes = build_frame(&s, 1, 0, 0, 0, 3, None);
        // Dois estilos distintos citados, ainda que três células os usem.
        assert_eq!(u32_em(&bytes, 28), 2);
        let q = ler(&bytes);
        assert_eq!(q.estilos, vec![4, 4, 9, SEM_ESTILO]);
    }

    #[test]
    fn texto_com_acento_e_medido_em_bytes() {
        let mut s = Sheet::new("Planilha1");
        s.set_cell_raw(0, 0, "São Paulo".into(), None);
        s.set_cell_raw(0, 1, "Ok".into(), None);
        let q = ler(&build_frame(&s, 1, 0, 0, 0, 1, None));
        // Se o tamanho fosse contado em caracteres, o segundo texto começaria
        // um byte antes e sairia "kO" ou coisa pior.
        assert_eq!(q.textos, vec!["São Paulo", "Ok"]);
    }

    #[test]
    fn as_colunas_de_u32_ficam_alinhadas() {
        // Com um número ÍMPAR de células, o bloco de bandeiras não fecha em
        // múltiplo de 4 sozinho. Sem o enchimento, o JavaScript não conseguiria
        // montar uma `Uint32Array` por cima do que vem depois.
        let mut s = Sheet::new("Planilha1");
        for c in 0..5 {
            s.set_cell_raw(0, c, format!("v{c}"), None);
        }
        let bytes = build_frame(&s, 1, 0, 0, 0, 4, None);
        let n = u32_em(&bytes, 24) as usize;
        assert_eq!(n, 5);
        let depois_das_bandeiras = INICIO_DAS_COLUNAS + n * 4 + n * 4 + n;
        let alinhado = depois_das_bandeiras.div_ceil(4) * 4;
        assert_eq!(alinhado % 4, 0);
        // E o que está lá é mesmo o índice de estilo da primeira célula.
        assert_eq!(u32_em(&bytes, alinhado), SEM_ESTILO);
    }

    #[test]
    fn tela_rolada_para_o_vazio_devolve_quadro_sem_celula() {
        let s = planilha();
        let q = ler(&build_frame(&s, 1, 900_000, 50, 900_040, 70, None));
        assert_eq!(q.n, 0);
        assert!(q.textos.is_empty());
    }

    #[test]
    fn a_moeda_atravessa_a_ponte_ja_formatada() {
        // A promessa do docs/ARQUITETURA.md: "o que atravessa a ponte é o texto
        // pronto para desenhar". Sem a tabela de estilos ligada, o quadro
        // levava "1250,5" onde a grade mostra "R$ 1.250,50".
        use crate::celulas::style::{NumberFormat, Style};
        let mut s = Sheet::new("Planilha1");
        let moeda = s
            .registrar_estilo(Style { format: NumberFormat::Currency, ..Default::default() })
            .unwrap();
        s.set_cell_raw(0, 0, "1250,5".into(), Some(moeda));
        let q = ler(&build_frame(&s, 1, 0, 0, 0, 0, None));
        assert_eq!(q.textos, vec!["R$\u{00A0}1.250,50"]);
        assert_eq!(q.bandeiras[0] & BANDEIRA_NUMERO, BANDEIRA_NUMERO);
    }

    #[test]
    fn a_data_atravessa_como_data() {
        use crate::celulas::style::{NumberFormat, Style};
        let mut s = Sheet::new("Planilha1");
        let data = s
            .registrar_estilo(Style { format: NumberFormat::Date, ..Default::default() })
            .unwrap();
        s.set_cell_raw(0, 0, "46000".into(), Some(data));
        let q = ler(&build_frame(&s, 1, 0, 0, 0, 0, None));
        assert_eq!(q.textos, vec!["09/12/2025"]);
    }

    #[test]
    fn estilo_repetido_ocupa_um_lugar_so() {
        use crate::celulas::style::Style;
        let mut s = Sheet::new("Planilha1");
        let negrito = Style { bold: true, ..Default::default() };
        let a = s.registrar_estilo(negrito.clone());
        let b = s.registrar_estilo(negrito);
        assert_eq!(a, b, "o mesmo estilo entrou duas vezes na tabela");
        assert_eq!(s.style_table.len(), 1);
        // E o estilo vazio não ocupa lugar nenhum.
        assert_eq!(s.registrar_estilo(Style::default()), None);
        assert_eq!(s.style_table.len(), 1);
    }
}
