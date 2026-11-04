//! Referência a outra pasta, como o vínculo externo do Excel (pedido de
//! 24/09/2026, retomado para a v0.1.0 em 16/10/2026): `=[Vendas.fshz]Dados!A1`
//! lê a célula A1 da aba Dados do arquivo `Vendas.fshz`. Também valem a
//! forma entre aspas (`='[Minhas Vendas.fshz]Dados'!A1`) e a com caminho
//! (`='/home/x/[Vendas.fshz]Dados'!A1`), como o Excel escreve.
//!
//! O analisador entende isso como uma aba cujo nome tem `[arquivo]`. O que
//! mora aqui é o resto:
//!
//! - separar o arquivo da aba ([`separar`]) e montar o nome de volta
//!   ([`nome_de_aba`]);
//! - guardar as pastas externas, **só com os valores** já calculados de cada
//!   aba ([`retrato_de_valores`]). A fórmula de lá não é recalculada aqui —
//!   e assim uma referência de lá para outra aba nunca é resolvida contra a
//!   pasta errada. Cada pasta guarda também de onde os valores vieram e
//!   quando ([`LivroExterno`]): é o que a interface mostra com a origem
//!   fechada, e o que o `.fshz` grava;
//! - anotar o arquivo que alguém pediu e ainda não está carregado. Quem lê o
//!   disco (ou conversa com a outra janela) é a interface: ela pega os
//!   pedidos, carrega e devolve com [`Externos::instalar`].
//!
//! O nome do arquivo vai como foi escrito: relativo à pasta do arquivo que o
//! cita, ou caminho absoluto. Quem resolve o caminho é a interface.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

use crate::celulas::refs::{key_col, key_row, Rect};
use crate::celulas::values::Value;
use crate::planilha::sheet::Sheet;
use crate::planilha::workbook::Workbook;

/// `[arquivo]Aba` → (`arquivo`, `Aba`); `pasta/[arquivo]Aba` →
/// (`pasta/arquivo`, `Aba`). `None` quando o nome não é de outro arquivo.
pub fn separar(nome: &str) -> Option<(Cow<'_, str>, &str)> {
    let abre = nome.find('[')?;
    let resto = &nome[abre + 1..];
    let fim = resto.find(']')?;
    let (arquivo, aba) = (&resto[..fim], &resto[fim + 1..]);
    if arquivo.is_empty() || aba.is_empty() || arquivo.contains('[') {
        return None;
    }
    let pasta = &nome[..abre];
    let arquivo = if pasta.is_empty() {
        Cow::Borrowed(arquivo)
    } else if pasta.ends_with('/') || pasta.ends_with('\\') {
        Cow::Owned(format!("{pasta}{arquivo}"))
    } else {
        Cow::Owned(format!("{pasta}/{arquivo}"))
    };
    Some((arquivo, aba))
}

/// Se o nome de aba é de outro arquivo (`[Vendas.fshz]Dados`).
pub fn e_externa(nome: &str) -> bool {
    nome.contains('[') && separar(nome).is_some()
}

/// O nome de aba que a fórmula usa para `aba` de `arquivo`: `[Vendas.fshz]Dados`,
/// ou `/home/x/[Vendas.fshz]Dados` quando o arquivo vem com pasta.
pub fn nome_de_aba(arquivo: &str, aba: &str) -> String {
    match arquivo.rfind(['/', '\\']) {
        Some(p) => format!("{}[{}]{aba}", &arquivo[..=p], &arquivo[p + 1..]),
        None => format!("[{arquivo}]{aba}"),
    }
}

/// A chave de um arquivo: sem diferença de maiúscula nem de barra. É assim
/// que duas fórmulas que escrevem o mesmo arquivo de jeitos diferentes
/// (`vendas.fshz`, `Vendas.fshz`) caem na mesma pasta externa.
pub fn chave_arquivo(arquivo: &str) -> String {
    arquivo.replace('\\', "/").to_lowercase()
}

/// Se dois nomes escritos falam do mesmo arquivo.
pub fn mesmo_arquivo(a: &str, b: &str) -> bool {
    chave_arquivo(a) == chave_arquivo(b)
}

/// A referência escrita na fórmula: sempre entre aspas
/// (`'[plan_b.plz]Planilha1'!A1:B2`), que vale para qualquer nome.
pub fn referencia(arquivo: &str, aba: &str, r: &Rect) -> String {
    let nome = nome_de_aba(arquivo, aba).replace('\'', "''");
    let a1 = crate::celulas::refs::cell_name(r.r0, r.c0);
    if r.r0 == r.r1 && r.c0 == r.c1 {
        format!("'{nome}'!{a1}")
    } else {
        format!("'{nome}'!{a1}:{}", crate::celulas::refs::cell_name(r.r1, r.c1))
    }
}

/// As abas de uma pasta só com os valores (cada fórmula vira o resultado
/// dela, calculado na pasta de origem).
pub fn retrato_de_valores(pasta: &Workbook) -> Vec<Sheet> {
    (0..pasta.len())
        .filter_map(|i| {
            let viva = pasta.aba(i)?;
            let mut copia = Sheet::new(viva.sheet.name.clone());
            let chaves = viva.sheet.chaves_no_retangulo(&Rect::new(0, 0, u32::MAX - 1, u32::MAX - 1));
            for k in chaves {
                let (r, c) = (key_row(k), key_col(k));
                copia.set_cell_valor(r, c, pasta.valor(i, r, c));
            }
            copia.rastro.borrow_mut().desconhecer();
            Some(copia)
        })
        .collect()
}

/// Uma aba só com valores, montada de células avulsas (o que a outra janela
/// manda ou o que o arquivo guardou).
pub fn aba_de_valores(nome: &str, celulas: &[(u32, u32, Value)]) -> Sheet {
    let mut s = Sheet::new(nome.to_string());
    for (r, c, v) in celulas {
        if !matches!(v, Value::Empty) {
            s.set_cell_valor(*r, *c, v.clone());
        }
    }
    s.rastro.borrow_mut().desconhecer();
    s
}

/// Os valores não vazios de uma aba só com valores dentro de `area`.
pub fn valores_na_area(aba: &Sheet, area: &Rect) -> Vec<(u32, u32, Value)> {
    let mut chaves = aba.chaves_no_retangulo(area);
    chaves.sort_unstable();
    chaves
        .into_iter()
        .map(|k| (key_row(k), key_col(k), aba.get_value(key_row(k), key_col(k))))
        .filter(|(_, _, v)| !matches!(v, Value::Empty))
        .collect()
}

/// Se duas listas de abas têm os mesmos nomes e os mesmos valores.
fn mesmas_abas(a: &[Sheet], b: &[Sheet]) -> bool {
    let tudo = Rect::new(0, 0, u32::MAX - 1, u32::MAX - 1);
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            if x.name != y.name {
                return false;
            }
            let (kx, ky) = (x.chaves_no_retangulo(&tudo), y.chaves_no_retangulo(&tudo));
            kx == ky && kx.iter().all(|&k| x.get_value(key_row(k), key_col(k)) == y.get_value(key_row(k), key_col(k)))
        })
}

/// Uma pasta externa: os valores e de onde/quando vieram.
pub struct LivroExterno {
    /// O arquivo como a fórmula escreveu (`Vendas.fshz`, `../x/Vendas.fshz`).
    pub nome: String,
    /// As abas, só com valores.
    pub abas: Vec<Sheet>,
    /// O último caminho absoluto de onde os valores vieram — é por ele que
    /// "Editar links" mostra a origem e que um nome relativo ainda acha o
    /// arquivo depois que a pasta de destino mudou de lugar.
    pub caminho: Option<String>,
    /// Quando os valores chegaram, em segundos desde 1970 (UTC).
    pub atualizado_em: Option<i64>,
}

impl LivroExterno {
    /// A aba com este nome, sem diferenciar maiúscula.
    pub fn aba(&self, nome: &str) -> Option<&Sheet> {
        self.abas.iter().find(|s| crate::planilha::sheet::mesmo_nome(&s.name, nome))
    }

    /// Os nomes das abas, na ordem.
    pub fn nomes_das_abas(&self) -> Vec<String> {
        self.abas.iter().map(|s| s.name.clone()).collect()
    }
}

/// As pastas externas de uma pasta, por [`chave_arquivo`].
#[derive(Default)]
pub struct Externos {
    livros: HashMap<String, LivroExterno>,
    /// Arquivos citados que ainda não foram carregados. `RefCell` porque é
    /// no meio do cálculo (com `&self`) que se descobre a falta.
    pedidos: RefCell<BTreeSet<String>>,
}

impl Externos {
    /// A aba externa com esse nome (`[arquivo]Aba`), se o arquivo já está
    /// carregado; senão anota o pedido. `None` também para nome que não é
    /// externo e para aba que a pasta carregada não tem (vira `#REF!`).
    pub fn aba(&self, nome: &str) -> Option<&Sheet> {
        let (arquivo, aba) = separar(nome)?;
        match self.livros.get(&chave_arquivo(&arquivo)) {
            Some(livro) => livro.aba(aba),
            None => {
                self.pedidos.borrow_mut().insert(arquivo.into_owned());
                None
            }
        }
    }

    /// Os arquivos que o cálculo pediu desde a última pergunta.
    pub fn tirar_pedidos(&self) -> Vec<String> {
        std::mem::take(&mut *self.pedidos.borrow_mut()).into_iter().collect()
    }

    /// Os arquivos já carregados (como foram escritos na chave), para a
    /// interface conferir se mudaram no disco.
    pub fn carregados(&self) -> Vec<String> {
        let mut v: Vec<String> = self.livros.values().map(|l| l.nome.clone()).collect();
        v.sort();
        v
    }

    pub fn livro(&self, arquivo: &str) -> Option<&LivroExterno> {
        self.livros.get(&chave_arquivo(arquivo))
    }

    /// Todas as pastas carregadas, em ordem de nome.
    pub fn livros(&self) -> Vec<&LivroExterno> {
        let mut v: Vec<&LivroExterno> = self.livros.values().collect();
        v.sort_by_key(|l| chave_arquivo(&l.nome));
        v
    }

    /// Troca valores avulsos de uma aba já carregada — o que a outra janela
    /// manda na hora da edição, sem esperar o arquivo ir ao disco. Responde
    /// se algum valor MUDOU: igual não conta, senão duas pastas que se citam
    /// ficariam avisando uma à outra para sempre. `false` também quando o
    /// arquivo não está carregado. Aba que ainda não existia na cópia é
    /// criada (a outra janela acabou de ganhar a aba).
    pub fn atualizar_celulas(&mut self, arquivo: &str, aba: &str, celulas: &[(u32, u32, Value)]) -> bool {
        let Some(livro) = self.livros.get_mut(&chave_arquivo(arquivo)) else { return false };
        let posicao = match livro.abas.iter().position(|s| crate::planilha::sheet::mesmo_nome(&s.name, aba)) {
            Some(p) => p,
            None => {
                livro.abas.push(aba_de_valores(aba, &[]));
                livro.abas.len() - 1
            }
        };
        let s = &mut livro.abas[posicao];
        let mut mudou = false;
        for (r, c, v) in celulas {
            if s.get_value(*r, *c) != *v {
                s.set_cell_valor(*r, *c, v.clone());
                mudou = true;
            }
        }
        mudou
    }

    /// Põe (ou troca) a pasta de um arquivo. Responde se mudou alguma coisa
    /// (pelo mesmo motivo do [`Self::atualizar_celulas`]). O caminho e a data
    /// guardados ficam como estavam.
    pub fn instalar(&mut self, arquivo: &str, abas: Vec<Sheet>) -> bool {
        let chave = chave_arquivo(arquivo);
        let (caminho, atualizado_em) = self.livros.get(&chave)
            .map_or((None, None), |l| (l.caminho.clone(), l.atualizado_em));
        self.instalar_com(arquivo, abas, caminho, atualizado_em)
    }

    /// [`Self::instalar`] dizendo de onde e quando os valores vieram.
    pub fn instalar_com(&mut self, arquivo: &str, abas: Vec<Sheet>, caminho: Option<String>, atualizado_em: Option<i64>) -> bool {
        let chave = chave_arquivo(arquivo);
        let igual = self.livros.get(&chave).is_some_and(|velho| mesmas_abas(&velho.abas, &abas));
        self.livros.insert(chave, LivroExterno { nome: arquivo.to_string(), abas, caminho, atualizado_em });
        !igual
    }

    /// Anota de onde e quando vieram os valores de uma pasta já carregada.
    pub fn marcar(&mut self, arquivo: &str, caminho: Option<String>, atualizado_em: Option<i64>) {
        if let Some(livro) = self.livros.get_mut(&chave_arquivo(arquivo)) {
            if caminho.is_some() {
                livro.caminho = caminho;
            }
            if atualizado_em.is_some() {
                livro.atualizado_em = atualizado_em;
            }
        }
    }

    /// Tira uma pasta (ninguém mais a cita, ou o link foi trocado).
    pub fn remover(&mut self, arquivo: &str) -> Option<LivroExterno> {
        self.livros.remove(&chave_arquivo(arquivo))
    }

    /// Nenhuma pasta carregada e nenhum pedido: a pasta não cita outro
    /// arquivo (ou ainda não calculou a fórmula que cita).
    pub fn vazio(&self) -> bool {
        self.livros.is_empty() && self.pedidos.borrow().is_empty()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::{CellError, ErrorCode};

    fn par<'a>(v: Option<(Cow<'a, str>, &'a str)>) -> Option<(String, &'a str)> {
        v.map(|(a, b)| (a.into_owned(), b))
    }

    #[test]
    fn separa_arquivo_e_aba() {
        assert_eq!(par(separar("[plan_b.plz]Planilha1")), Some(("plan_b.plz".into(), "Planilha1")));
        assert_eq!(par(separar("[/home/x/plan b.plz]Meu Plano")), Some(("/home/x/plan b.plz".into(), "Meu Plano")));
        // A forma do Excel: a pasta fica antes do colchete.
        assert_eq!(par(separar("/home/x/[Vendas.fshz]Dados")), Some(("/home/x/Vendas.fshz".into(), "Dados")));
        assert_eq!(par(separar("C:\\Planilhas\\[Vendas.xlsx]Dados")), Some(("C:\\Planilhas\\Vendas.xlsx".into(), "Dados")));
        assert_eq!(separar("Planilha1"), None);
        assert_eq!(separar("[]Aba"), None);
        assert_eq!(separar("[x.plz]"), None);
        assert!(e_externa("[Vendas.fshz]Dados"));
        assert!(!e_externa("Dados"));
    }

    #[test]
    fn monta_o_nome_da_aba_de_volta() {
        assert_eq!(nome_de_aba("Vendas.fshz", "Dados"), "[Vendas.fshz]Dados");
        assert_eq!(nome_de_aba("/home/x/Vendas.fshz", "Dados"), "/home/x/[Vendas.fshz]Dados");
        for nome in ["[Vendas.fshz]Dados", "/home/x/[Vendas.fshz]Dados"] {
            let (arquivo, aba) = separar(nome).unwrap();
            assert_eq!(nome_de_aba(&arquivo, aba), nome);
        }
        assert!(mesmo_arquivo("Vendas.FSHZ", "vendas.fshz"));
        assert!(mesmo_arquivo("C:\\x\\V.xlsx", "c:/x/v.xlsx"));
    }

    #[test]
    fn escreve_a_referencia_entre_aspas() {
        assert_eq!(referencia("plan_b.plz", "Planilha1", &Rect::new(0, 0, 0, 0)), "'[plan_b.plz]Planilha1'!A1");
        assert_eq!(referencia("d'água.plz", "P", &Rect::new(1, 1, 3, 2)), "'[d''água.plz]P'!B2:C4");
    }

    /// A outra janela manda só as células que mudaram: a A recalcula sem
    /// recarregar a pasta inteira.
    #[test]
    fn valores_avulsos_da_outra_janela() {
        let mut b = Workbook::new();
        b.escrever_celula(0, 0, 0, "10".into());
        let mut a = Workbook::new();
        a.escrever_celula(0, 0, 0, "='[plan_b.plz]Planilha1'!A1+1".into());
        let _ = a.valor(0, 0, 0);
        a.instalar_externo("plan_b.plz", retrato_de_valores(&b));
        assert_eq!(a.valor(0, 0, 0), Value::Number(11.0));
        assert!(a.atualizar_externo("plan_b.plz", "planilha1", &[(0, 0, Value::Number(99.0))]));
        assert_eq!(a.valor(0, 0, 0), Value::Number(100.0));
        // O mesmo valor de novo não é mudança (duas pastas que se citam
        // não podem ficar avisando uma à outra para sempre).
        let versao = a.versao(0);
        assert!(!a.atualizar_externo("plan_b.plz", "planilha1", &[(0, 0, Value::Number(99.0))]));
        assert_eq!(a.versao(0), versao);
        assert!(!a.instalar_externo("plan_b.plz", retrato_de_valores(&{
            let mut b2 = Workbook::new();
            b2.escrever_celula(0, 0, 0, "99".into());
            b2
        })), "a mesma pasta de novo não muda nada");
        assert!(!a.atualizar_externo("outro.plz", "Planilha1", &[]), "arquivo não carregado");
    }

    /// De ponta a ponta: a pasta A lê a B, que chega depois (como a interface faz).
    #[test]
    fn a_formula_le_a_outra_pasta_quando_ela_chega() {
        let mut b = Workbook::new();
        b.escrever_celula(0, 0, 0, "10".into());
        b.escrever_celula(0, 1, 0, "=A1*3".into());
        let mut a = Workbook::new();
        a.escrever_celula(0, 0, 0, "='[plan_b.plz]Planilha1'!A2+1".into());
        a.escrever_celula(0, 1, 0, "=SOMA('[plan_b.plz]Planilha1'!A1:A2)".into());
        // Antes de carregar: #REF!, e o pedido fica anotado.
        assert!(matches!(a.valor(0, 0, 0), Value::Error(_)));
        assert!(matches!(a.valor(0, 1, 0), Value::Error(_)), "intervalo de arquivo sem valores também é #REF!");
        assert_eq!(a.externos_pedidos(), vec!["plan_b.plz".to_string()]);
        a.instalar_externo("plan_b.plz", retrato_de_valores(&b));
        assert_eq!(a.valor(0, 0, 0), Value::Number(31.0));
        assert_eq!(a.valor(0, 1, 0), Value::Number(40.0));
        // A B mudou no disco: a nova versão chega e a A recalcula.
        b.escrever_celula(0, 0, 0, "20".into());
        a.instalar_externo("plan_b.plz", retrato_de_valores(&b));
        assert_eq!(a.valor(0, 0, 0), Value::Number(61.0));
        assert_eq!(a.valor(0, 1, 0), Value::Number(80.0));
    }

    /// As formas novas: sem aspas, intervalo e com caminho entre aspas.
    #[test]
    fn referencia_sem_aspas_intervalo_e_caminho() {
        let mut a = Workbook::new();
        a.escrever_celula(0, 0, 0, "=[Vendas.fshz]Dados!A1".into());
        a.escrever_celula(0, 1, 0, "=SOMA([Vendas.fshz]Dados!A1:A10)".into());
        a.escrever_celula(0, 2, 0, "='/home/x/[Vendas.fshz]Dados'!A2*2".into());
        a.escrever_celula(0, 3, 0, "=[Vendas.fshz]Outra!A1".into());
        let dados = aba_de_valores("Dados", &[(0, 0, Value::Number(5.0)), (1, 0, Value::Number(7.0))]);
        a.instalar_externo_com("Vendas.fshz", vec![dados], Some("/home/x/Vendas.fshz".into()), Some(1_760_630_000));
        let dados2 = aba_de_valores("Dados", &[(1, 0, Value::Number(8.0))]);
        a.instalar_externo_com("/home/x/Vendas.fshz", vec![dados2], None, None);
        assert_eq!(a.valor(0, 0, 0), Value::Number(5.0));
        assert_eq!(a.valor(0, 1, 0), Value::Number(12.0));
        assert_eq!(a.valor(0, 2, 0), Value::Number(16.0));
        // Aba que o arquivo não tem: #REF!.
        assert_eq!(a.valor(0, 3, 0), Value::Error(CellError::new(ErrorCode::Ref)));
        let livro = a.livro_externo("vendas.FSHZ").expect("carregado");
        assert_eq!(livro.atualizado_em, Some(1_760_630_000));
        assert_eq!(livro.caminho.as_deref(), Some("/home/x/Vendas.fshz"));
    }
}
