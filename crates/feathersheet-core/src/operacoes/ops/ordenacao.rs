//! Ordenação de intervalos.

use crate::celulas::refs::Rect;
use crate::celulas::values::Value;
use crate::planilha::sheet::Sheet;

/// Uma chave de ordenação: a coluna (índice absoluto) e o sentido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortKey {
    pub col: u32,
    pub ascending: bool,
}

/// O que o `sortRange` decidiu: da linha `first` à `max_r`, `ordem[i]` é a
/// linha que vai parar em `first + i`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanoDeOrdem {
    pub first: u32,
    pub max_r: u32,
    pub ordem: Vec<u32>,
}

/// A metade que lê do `sortRange`. `None` quando não há o que mexer (uma
/// linha só, nenhuma chave, ou já estava em ordem).
pub fn planejar_ordenacao(
    area: Rect,
    keys: &[SortKey],
    has_header: bool,
    max_row: u32,
    valor: impl Fn(u32, u32) -> Value,
) -> Option<PlanoDeOrdem> {
    planejar_ordenacao_com(area, keys, has_header, max_row, false, valor)
}

/// Desempate do "Diferenciar maiúsculas e minúsculas" do Excel: textos iguais
/// sem olhar a caixa ficam com a minúscula antes ("abc" antes de "ABC").
pub(crate) fn desempate_por_caixa(a: &Value, b: &Value) -> std::cmp::Ordering {
    let (Value::Text(a), Value::Text(b)) = (a, b) else { return std::cmp::Ordering::Equal };
    let chave = |t: &str| t.chars().map(|c| c.is_uppercase()).collect::<Vec<_>>();
    chave(a).cmp(&chave(b))
}

/// [`planejar_ordenacao`] com a opção de diferenciar maiúsculas (a janela
/// Classificar). Sem ela, "abc" e "ABC" empatam e mantêm a ordem original.
pub fn planejar_ordenacao_com(
    area: Rect,
    keys: &[SortKey],
    has_header: bool,
    max_row: u32,
    maiusculas: bool,
    valor: impl Fn(u32, u32) -> Value,
) -> Option<PlanoDeOrdem> {
    let r = area.normalized();
    let first = r.r0 + u32::from(has_header);
    let max_r = r.r1.min(max_row);
    if first >= max_r || keys.is_empty() {
        return None;
    }
    // O TypeScript lê o valor dentro da comparação; ler uma vez por linha dá
    // o mesmo resultado sem as O(n log n) leituras.
    let valores: Vec<Vec<Value>> = keys.iter().map(|k| (first..=max_r).map(|row| valor(row, k.col)).collect()).collect();
    let mut ordem: Vec<u32> = (first..=max_r).collect();
    ordem.sort_by(|&a, &b| {
        for (ki, k) in keys.iter().enumerate() {
            let (va, vb) = (&valores[ki][(a - first) as usize], &valores[ki][(b - first) as usize]);
            // Com erro de um dos lados, a chave não decide (o `typeof res
            // === 'number'` do TypeScript).
            if let Ok(mut o) = crate::celulas::values::compare_exato(va, vb) {
                if o == std::cmp::Ordering::Equal && maiusculas { o = desempate_por_caixa(va, vb); }
                if o != std::cmp::Ordering::Equal {
                    return if k.ascending { o } else { o.reverse() };
                }
            }
        }
        a.cmp(&b)
    });
    if ordem.iter().enumerate().all(|(i, &row)| row == first + i as u32) {
        return None;
    }
    Some(PlanoDeOrdem { first, max_r, ordem })
}

/// A metade que escreve do `sortRange`: as linhas vão para o lugar novo com
/// o estilo, e a fórmula anda junto. Direto no armazenamento, um passo só no
/// desfazer — ver [`Sheet::permutar_linhas`].
pub fn aplicar_ordenacao(sheet: &mut Sheet, c0: u32, c1: u32, plano: &PlanoDeOrdem) {
    sheet.permutar_linhas(plano.first, c0, c1, &plano.ordem);
}

/// `sortRange`, lendo o `Sheet` sozinho.
pub fn sort_range(sheet: &mut Sheet, area: Rect, keys: &[SortKey], has_header: bool) {
    let max_row = sheet.max_row_ts();
    let plano = {
        let s = &*sheet;
        planejar_ordenacao(area.clone(), keys, has_header, max_row, |r, c| s.get_value(r, c))
    };
    if let Some(p) = plano {
        let r = area.normalized();
        aplicar_ordenacao(sheet, r.c0, r.c1, &p);
    }
}

