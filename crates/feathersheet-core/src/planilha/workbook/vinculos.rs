//! A pasta e os outros arquivos que as fórmulas dela citam
//! (`=[Vendas.fshz]Dados!A1`): quais áreas cada fórmula lê, os valores que
//! chegam de fora (da outra janela ou do disco) e o que o `.fshz` guarda.
//!
//! O motor não sabe de janela nem de disco: a interface descobre de onde vêm
//! os valores e entrega por [`Workbook::instalar_externo_com`] (a pasta
//! inteira, ou só as áreas citadas) e [`Workbook::atualizar_externo_em`] (só
//! as células que mudaram lá). Em ambos, só as fórmulas que leem aquela
//! área (e quem depende delas) voltam a calcular.

use std::collections::BTreeSet;

use super::{proxima_versao, reescrever_aba, Workbook};
use crate::celulas::refs::{key_col, key_row, Rect, MAX_COLS, MAX_ROWS};
use crate::celulas::values::Value;
use crate::formula::ast::Node;
use crate::planilha::sheet::Sheet;
use crate::planilha::vinculo::{self, chave_arquivo, separar, LivroExterno};

/// Uma área de outro arquivo lida por alguma fórmula da pasta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AreaCitada {
    /// O arquivo como a fórmula escreveu.
    pub arquivo: String,
    pub aba: String,
    /// Normalizada, sem aba (`sheet: None`).
    pub area: Rect,
}

/// O que o arquivo de destino guarda de uma pasta externa: só as células
/// das áreas citadas, com a origem e a data dos valores.
#[derive(Clone, Debug, PartialEq)]
pub struct LivroGuardado {
    pub nome: String,
    pub caminho: Option<String>,
    pub atualizado_em: Option<i64>,
    /// (aba, células não vazias).
    pub abas: Vec<(String, Vec<(u32, u32, Value)>)>,
}

/// As células avulsas de uma atualização viram retângulos para sujar só
/// quem lê ali. Muitas células: o retângulo que cobre todas (sujar a mais é
/// seguro; comparar milhares de retângulos por fórmula, não).
fn retangulos(celulas: &[(u32, u32, Value)]) -> Vec<Rect> {
    if celulas.len() <= 64 {
        return celulas.iter().map(|(r, c, _)| Rect::new(*r, *c, *r, *c)).collect();
    }
    let (mut r0, mut c0, mut r1, mut c1) = (u32::MAX, u32::MAX, 0, 0);
    for (r, c, _) in celulas {
        r0 = r0.min(*r);
        c0 = c0.min(*c);
        r1 = r1.max(*r);
        c1 = c1.max(*c);
    }
    vec![Rect::new(r0, c0, r1, c1)]
}

/// Percorre as referências a outro arquivo de uma fórmula: (nome da aba como
/// escrito, área normalizada).
fn refs_externas(no: &Node, visitar: &mut dyn FnMut(&str, Rect)) {
    no.walk(&mut |n| match n {
        Node::Ref { sheet: Some(s), row, col } if vinculo::e_externa(s) => {
            visitar(s, Rect::new(*row, *col, *row, *col));
        }
        Node::Range { sheet: Some(s), r0, c0, r1, c1 } if vinculo::e_externa(s) => {
            visitar(s, Rect::new(*r0.min(r1), *c0.min(c1), *r0.max(r1), *c0.max(c1)));
        }
        _ => {}
    });
}

/// A árvore de uma fórmula avulsa (a guardada, ou o texto analisado de novo:
/// a árvore é solta depois do cálculo).
fn com_arvore(sheet: &Sheet, k: u64, usar: &mut dyn FnMut(&Node)) {
    let Some(celula) = sheet.cells.get(&k) else { return };
    match &celula.formula {
        Some(no) => usar(no),
        None => {
            if let Some(Ok(no)) = celula.raw.strip_prefix('=').map(crate::formula::parser::parse_formula) {
                usar(&no);
            }
        }
    }
}

impl Workbook {
    /// As áreas de outros arquivos que as fórmulas leem, sem repetição (uma
    /// área dentro de outra do mesmo arquivo e aba some), em ordem estável.
    pub fn areas_citadas(&self) -> Vec<AreaCitada> {
        // Só muda quando uma fórmula de vínculo entra ou sai de alguma aba.
        let geracoes: Vec<(u32, u64)> = self.abas.iter().map(|a| (a.id, a.sheet.geracao_externas())).collect();
        if let Some((g, areas)) = self.areas_cache.borrow().as_ref() {
            if *g == geracoes {
                return areas.clone();
            }
        }
        let todas = self.areas_citadas_sem_guardar();
        *self.areas_cache.borrow_mut() = Some((geracoes, todas.clone()));
        todas
    }

    fn areas_citadas_sem_guardar(&self) -> Vec<AreaCitada> {
        let mut todas: Vec<AreaCitada> = Vec::new();
        for aba in &self.abas {
            for &k in &aba.sheet.externas {
                com_arvore(&aba.sheet, k, &mut |no| {
                    refs_externas(no, &mut |nome, area| {
                        if let Some((arquivo, aba)) = separar(nome) {
                            todas.push(AreaCitada { arquivo: arquivo.into_owned(), aba: aba.to_string(), area });
                        }
                    });
                });
            }
            for (_, nome, area) in aba.sheet.vinculos_empacotados() {
                if let Some((arquivo, aba_ext)) = separar(&nome) {
                    todas.push(AreaCitada { arquivo: arquivo.into_owned(), aba: aba_ext.to_string(), area });
                }
            }
        }
        let ordem = |a: &AreaCitada| (chave_arquivo(&a.arquivo), a.aba.to_lowercase(), a.area.r0, a.area.c0, a.area.r1, a.area.c1);
        todas.sort_by_key(ordem);
        todas.dedup_by(|a, b| ordem(&*a) == ordem(&*b));
        let dentro = |a: &Rect, b: &Rect| b.r0 <= a.r0 && a.r1 <= b.r1 && b.c0 <= a.c0 && a.c1 <= b.c1;
        let copia = todas.clone();
        todas.retain(|a| {
            !copia.iter().any(|b| {
                b != a
                    && chave_arquivo(&b.arquivo) == chave_arquivo(&a.arquivo)
                    && b.aba.to_lowercase() == a.aba.to_lowercase()
                    && dentro(&a.area, &b.area)
            })
        });
        todas
    }

    /// Os arquivos citados (como a primeira fórmula escreveu), sem repetição.
    pub fn origens_citadas(&self) -> Vec<String> {
        let mut vistos = BTreeSet::new();
        self.areas_citadas()
            .into_iter()
            .filter(|a| vistos.insert(chave_arquivo(&a.arquivo)))
            .map(|a| a.arquivo)
            .collect()
    }

    /// Se alguma fórmula já leu outro arquivo (com ou sem valores). Barato:
    /// a interface pergunta a cada mensagem para decidir se vigia vínculos.
    pub fn tem_vinculos_externos(&self) -> bool {
        !self.externos.vazio()
    }

    /// A pasta externa carregada deste arquivo.
    pub fn livro_externo(&self, arquivo: &str) -> Option<&LivroExterno> {
        self.externos.livro(arquivo)
    }

    /// Todas as pastas externas carregadas.
    pub fn livros_externos(&self) -> Vec<&LivroExterno> {
        self.externos.livros()
    }

    /// Põe (ou troca) os valores de uma pasta externa, dizendo de onde e
    /// quando vieram, e recalcula só quem lê aquele arquivo. Responde se
    /// algum valor mudou.
    pub fn instalar_externo_com(&mut self, arquivo: &str, abas: Vec<Sheet>, caminho: Option<String>, atualizado_em: Option<i64>) -> bool {
        let mudou = self.externos.instalar_com(arquivo, abas, caminho, atualizado_em);
        if mudou {
            self.sujar_quem_le_vinculo(arquivo, None, None);
        }
        mudou
    }

    /// Troca células avulsas de uma pasta externa (a outra janela mandou o
    /// que mudou lá) e recalcula só as fórmulas que leem essas células, e
    /// quem depende delas. `false` quando nada mudou ou o arquivo não está
    /// carregado.
    pub fn atualizar_externo_em(&mut self, arquivo: &str, aba: &str, celulas: &[(u32, u32, Value)], atualizado_em: Option<i64>) -> bool {
        let mudou = self.externos.atualizar_celulas(arquivo, aba, celulas);
        self.externos.marcar(arquivo, None, atualizado_em);
        if mudou {
            let rects = retangulos(celulas);
            self.sujar_quem_le_vinculo(arquivo, Some(aba), Some(&rects));
        }
        mudou
    }

    /// Anota de onde e quando vieram os valores de uma pasta já carregada.
    pub fn marcar_externo(&mut self, arquivo: &str, caminho: Option<String>, atualizado_em: Option<i64>) {
        self.externos.marcar(arquivo, caminho, atualizado_em);
    }

    /// Suja, em todas as abas, as fórmulas que leem `arquivo` (na `aba`, ou
    /// em qualquer uma) dentro de `rects` (ou em qualquer lugar), e depois,
    /// aba a aba, quem lê uma aba que acabou de ficar suja. Sobe a versão de
    /// cada aba tocada. Responde se sujou alguma coisa.
    pub(crate) fn sujar_quem_le_vinculo(&mut self, arquivo: &str, aba: Option<&str>, rects: Option<&[Rect]>) -> bool {
        let mut fila: Vec<usize> = Vec::new();
        for (i, a) in self.abas.iter_mut().enumerate() {
            if a.sheet.sujar_vinculo(arquivo, aba, rects) {
                a.versao = proxima_versao(a.versao);
                fila.push(i);
            }
        }
        let sujou = !fila.is_empty();
        // Uma aba que leu de fora mudou: quem a lê, em outra aba, também
        // pode ter mudado. Cada aba se espalha uma vez só (a primeira vez já
        // suja tudo o que lê aquela aba, em qualquer coluna).
        let tudo = [Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1)];
        let mut espalhadas: BTreeSet<usize> = BTreeSet::new();
        while let Some(i) = fila.pop() {
            if !espalhadas.insert(i) {
                continue;
            }
            let nome = self.abas[i].sheet.name.clone();
            for (j, a) in self.abas.iter_mut().enumerate() {
                if j == i || !a.sheet.tem_formulas_externas() {
                    continue;
                }
                if a.sheet.sujar_externas_de(&nome, &tudo) {
                    a.versao = proxima_versao(a.versao);
                    fila.push(j);
                }
            }
        }
        sujou
    }

    /// O que o `.fshz` de destino guarda das pastas externas: para cada
    /// arquivo citado e carregado, só as células das áreas que as fórmulas
    /// leem. É o "último valor" que vale com a origem fechada.
    pub fn externos_para_gravar(&self) -> Vec<LivroGuardado> {
        let areas = self.areas_citadas();
        let mut saida: Vec<LivroGuardado> = Vec::new();
        for area in &areas {
            let Some(livro) = self.externos.livro(&area.arquivo) else { continue };
            let Some(aba) = livro.aba(&area.aba) else { continue };
            let chave = chave_arquivo(&area.arquivo);
            let posicao = match saida.iter().position(|g| chave_arquivo(&g.nome) == chave) {
                Some(p) => p,
                None => {
                    saida.push(LivroGuardado {
                        nome: livro.nome.clone(),
                        caminho: livro.caminho.clone(),
                        atualizado_em: livro.atualizado_em,
                        abas: Vec::new(),
                    });
                    saida.len() - 1
                }
            };
            let guardado = &mut saida[posicao];
            let celulas = match guardado.abas.iter().position(|(n, _)| crate::planilha::sheet::mesmo_nome(n, &aba.name)) {
                Some(p) => &mut guardado.abas[p].1,
                None => {
                    guardado.abas.push((aba.name.clone(), Vec::new()));
                    &mut guardado.abas.last_mut().expect("acabou de entrar").1
                }
            };
            celulas.extend(vinculo::valores_na_area(aba, &area.area));
        }
        for guardado in &mut saida {
            for (_, celulas) in &mut guardado.abas {
                celulas.sort_by_key(|(r, c, _)| (*r, *c));
                celulas.dedup_by_key(|(r, c, _)| (*r, *c));
            }
        }
        saida
    }

    /// Instala o que o `.fshz` guardou das pastas externas (na abertura).
    /// Os valores guardados no próprio arquivo, na abertura: sem sujar quem os lê, porque o
    /// resultado que o arquivo traz de cada fórmula foi calculado com esses mesmos valores. Sujar
    /// 780 mil fórmulas de vínculo e quem depende delas levava ~10 s ao abrir (#136).
    pub fn instalar_externos_na_leitura(&mut self, livros: Vec<LivroGuardado>) {
        for livro in livros {
            let abas = livro.abas.iter().map(|(nome, celulas)| vinculo::aba_de_valores(nome, celulas)).collect();
            self.externos.instalar_com(&livro.nome, abas, livro.caminho, livro.atualizado_em);
        }
    }

    pub fn instalar_externos_guardados(&mut self, livros: Vec<LivroGuardado>) {
        for livro in livros {
            let abas = livro.abas.iter().map(|(nome, celulas)| vinculo::aba_de_valores(nome, celulas)).collect();
            self.instalar_externo_com(&livro.nome, abas, livro.caminho, livro.atualizado_em);
        }
    }

    /// "Editar links → Trocar origem": reescreve as fórmulas que citam
    /// `antigo` para citar `novo` (`[antigo]Aba` vira `[novo]Aba`, com ou sem
    /// pasta), em todas as abas. Os valores que já havia continuam valendo
    /// até chegarem os da nova origem. Devolve quantas fórmulas mudaram.
    pub fn trocar_origem_externa(&mut self, antigo: &str, novo: &str) -> usize {
        self.trocar_origem_com(antigo, novo, true)
    }

    fn trocar_origem_com(&mut self, antigo: &str, novo: &str, desfazer: bool) -> usize {
        let alvo = chave_arquivo(antigo);
        let mut total = 0;
        for i in 0..self.abas.len() {
            let mut edicoes: Vec<(u32, u32, String)> = Vec::new();
            {
                let sheet = &self.abas[i].sheet;
                let mut chaves: Vec<u64> = sheet.externas.iter().copied().collect();
                chaves.sort_unstable();
                for k in chaves {
                    // Os nomes de aba (como escritos) que apontam para `antigo`.
                    let mut nomes: BTreeSet<String> = BTreeSet::new();
                    com_arvore(sheet, k, &mut |no| {
                        refs_externas(no, &mut |nome, _| {
                            if separar(nome).is_some_and(|(arquivo, _)| chave_arquivo(&arquivo) == alvo) {
                                nomes.insert(nome.to_string());
                            }
                        });
                    });
                    if nomes.is_empty() {
                        continue;
                    }
                    let (r, c) = (key_row(k), key_col(k));
                    let antes = sheet.get_raw(r, c);
                    let mut depois = antes.clone();
                    for nome in &nomes {
                        if let Some((_, aba)) = separar(nome) {
                            depois = reescrever_aba(&depois, nome, &vinculo::nome_de_aba(novo, aba));
                        }
                    }
                    if depois != antes {
                        edicoes.push((r, c, depois));
                    }
                }
                // Empacotadas: os nomes de aba vêm das referências do padrão da coluna.
                let mut por_coluna: std::collections::BTreeMap<u32, BTreeSet<String>> = Default::default();
                for (col, nome, _) in sheet.vinculos_empacotados() {
                    if separar(&nome).is_some_and(|(arquivo, _)| chave_arquivo(&arquivo) == alvo) {
                        por_coluna.entry(col).or_default().insert(nome);
                    }
                }
                for (col, nomes) in por_coluna {
                    let mut linhas = Vec::new();
                    sheet.formulas.for_each_in_range(0, MAX_ROWS - 1, col, col, |r, _| linhas.push(r));
                    for r in linhas {
                        let antes = sheet.get_raw(r, col);
                        let mut depois = antes.clone();
                        for nome in &nomes {
                            if let Some((_, aba)) = separar(nome) {
                                depois = reescrever_aba(&depois, nome, &vinculo::nome_de_aba(novo, aba));
                            }
                        }
                        if depois != antes {
                            edicoes.push((r, col, depois));
                        }
                    }
                }
            }
            if edicoes.is_empty() {
                continue;
            }
            total += edicoes.len();
            let aba = &mut self.abas[i];
            aba.sheet.abrir_transacao();
            // Uma cascata só no fim: uma por fórmula reescrita levava minutos ao abrir um
            // `.xlsx` com milhões de fórmulas lendo `[1]Aba!` (#136).
            aba.sheet.adiar_cascata();
            for (r, c, raw) in edicoes {
                if desfazer {
                    aba.sheet.set_cell(r, c, &raw);
                } else {
                    let style = aba.sheet.style_for(r, c, &raw);
                    aba.sheet.apply(r, c, Some(crate::planilha::sheet::edicao::Snap { raw, style }), false);
                }
            }
            aba.sheet.soltar_cascata();
            aba.sheet.fechar_transacao();
            aba.versao = proxima_versao(aba.versao);
        }
        if let Some(livro) = self.externos.remover(antigo) {
            self.externos.instalar_com(novo, livro.abas, None, livro.atualizado_em);
        }
        self.sujar_quem_le_vinculo(novo, None, None);
        total
    }

    /// O lado da origem: os valores não vazios de `area` na aba `aba` desta
    /// pasta (calculados agora), para mandar à janela que os cita. `None`
    /// quando a aba não existe.
    pub fn valores_da_area(&self, aba: &str, area: &Rect) -> Option<Vec<(u32, u32, Value)>> {
        let indice = self.indice_por_nome(aba)?;
        let sheet = &self.abas[indice].sheet;
        let mut chaves = sheet.chaves_no_retangulo(area);
        chaves.sort_unstable();
        Some(
            chaves
                .into_iter()
                .map(|k| (key_row(k), key_col(k), self.valor(indice, key_row(k), key_col(k))))
                .filter(|(_, _, v)| !matches!(v, Value::Empty))
                .collect(),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::{CellError, ErrorCode};

    fn pasta_que_cita() -> Workbook {
        let mut a = Workbook::new();
        a.escrever_celula(0, 0, 0, "=[Vendas.fshz]Dados!A1*2".into());
        a.escrever_celula(0, 1, 0, "=SOMA([Vendas.fshz]Dados!B1:B3)".into());
        a.escrever_celula(0, 2, 0, "=A1+1".into());
        a.escrever_celula(0, 3, 0, "=[Outro.fshz]Plan1!C5".into());
        a
    }

    fn dados() -> Sheet {
        vinculo::aba_de_valores("Dados", &[
            (0, 0, Value::Number(10.0)),
            (0, 1, Value::Number(1.0)),
            (1, 1, Value::Number(2.0)),
            (2, 1, Value::Number(3.0)),
            (9, 9, Value::Number(99.0)),
        ])
    }

    #[test]
    fn lista_as_areas_citadas_sem_repetir() {
        let mut a = pasta_que_cita();
        a.escrever_celula(0, 4, 0, "=[vendas.fshz]Dados!B2".into());
        let areas = a.areas_citadas();
        assert_eq!(areas.len(), 3, "{areas:?}");
        assert_eq!(areas.iter().filter(|x| chave_arquivo(&x.arquivo) == "vendas.fshz").count(), 2);
        assert!(areas.iter().any(|x| x.aba == "Dados" && x.area == Rect::new(0, 1, 2, 1)));
        assert_eq!(a.origens_citadas().len(), 2);
    }

    /// O provedor de valores externos: valor, intervalo, aba que não existe
    /// e arquivo sem valores.
    #[test]
    fn avalia_com_valores_de_fora() {
        let mut a = pasta_que_cita();
        assert!(matches!(a.valor(0, 0, 0), Value::Error(_)), "sem valores: #REF!");
        assert!(a.instalar_externo_com("Vendas.fshz", vec![dados()], Some("/x/Vendas.fshz".into()), Some(100)));
        assert_eq!(a.valor(0, 0, 0), Value::Number(20.0));
        assert_eq!(a.valor(0, 1, 0), Value::Number(6.0));
        assert_eq!(a.valor(0, 2, 0), Value::Number(21.0));
        assert_eq!(a.valor(0, 3, 0), Value::Error(CellError::new(ErrorCode::Ref)), "arquivo nunca carregado");
        a.instalar_externo_com("Outro.fshz", vec![vinculo::aba_de_valores("Resumo", &[])], None, None);
        assert_eq!(a.valor(0, 3, 0), Value::Error(CellError::new(ErrorCode::Ref)), "aba que o arquivo não tem");
    }

    /// Chegou uma mudança da origem: só quem lê a célula mudada recalcula.
    #[test]
    fn atualizacao_suja_so_as_dependentes() {
        let mut a = pasta_que_cita();
        a.instalar_externo_com("Vendas.fshz", vec![dados()], None, None);
        for r in 0..4 {
            let _ = a.valor(0, r, 0);
        }
        // B9 não é lida por ninguém: nada recalcula, nem a versão sobe.
        let versao = a.versao(0);
        assert!(a.atualizar_externo_em("Vendas.fshz", "Dados", &[(8, 1, Value::Number(5.0))], Some(200)));
        assert_eq!(a.versao(0), versao, "célula que ninguém lê não suja nada");
        assert!(!a.calculo_pendente(0, 0, 0));
        assert!(!a.calculo_pendente(0, 1, 0));
        // B2 entra só na SOMA.
        assert!(a.atualizar_externo_em("Vendas.fshz", "Dados", &[(1, 1, Value::Number(20.0))], Some(300)));
        assert!(a.calculo_pendente(0, 1, 0), "a SOMA lê B2");
        assert!(!a.calculo_pendente(0, 0, 0), "A1*2 não lê B2");
        assert!(!a.calculo_pendente(0, 3, 0), "outro arquivo");
        assert_eq!(a.valor(0, 1, 0), Value::Number(24.0));
        // A1 muda: A1*2 e quem depende dela (=A1+1) recalculam.
        assert!(a.atualizar_externo_em("Vendas.fshz", "dados", &[(0, 0, Value::Number(50.0))], None));
        assert!(a.calculo_pendente(0, 0, 0));
        assert!(a.calculo_pendente(0, 2, 0));
        assert!(!a.calculo_pendente(0, 1, 0));
        assert_eq!(a.valor(0, 2, 0), Value::Number(101.0));
        assert_eq!(a.livro_externo("Vendas.fshz").and_then(|l| l.atualizado_em), Some(300));
    }

    /// Outra aba que lê a aba que leu de fora também recalcula.
    #[test]
    fn mudanca_de_fora_chega_a_outra_aba() {
        let mut a = pasta_que_cita();
        a.criar_aba("Resumo");
        a.escrever_celula(1, 0, 0, "=Planilha1!A3*10".into());
        a.instalar_externo_com("Vendas.fshz", vec![dados()], None, None);
        assert_eq!(a.valor(1, 0, 0), Value::Number(210.0));
        a.atualizar_externo_em("Vendas.fshz", "Dados", &[(0, 0, Value::Number(1.0))], None);
        assert_eq!(a.valor(1, 0, 0), Value::Number(30.0));
    }

    /// O que vai para o arquivo: só as células citadas.
    #[test]
    fn guarda_so_as_celulas_citadas_e_reinstala() {
        let mut a = pasta_que_cita();
        a.instalar_externo_com("Vendas.fshz", vec![dados()], Some("/x/Vendas.fshz".into()), Some(123));
        let guardados = a.externos_para_gravar();
        assert_eq!(guardados.len(), 1, "Outro.fshz não tem valores");
        let g = &guardados[0];
        assert_eq!((g.nome.as_str(), g.caminho.as_deref(), g.atualizado_em), ("Vendas.fshz", Some("/x/Vendas.fshz"), Some(123)));
        assert_eq!(g.abas.len(), 1);
        let celulas: Vec<(u32, u32)> = g.abas[0].1.iter().map(|(r, c, _)| (*r, *c)).collect();
        assert_eq!(celulas, vec![(0, 0), (0, 1), (1, 1), (2, 1)], "J10 ninguém lê");
        let mut b = pasta_que_cita();
        b.instalar_externos_guardados(guardados);
        assert_eq!(b.valor(0, 0, 0), Value::Number(20.0));
        assert_eq!(b.valor(0, 1, 0), Value::Number(6.0));
    }

    #[test]
    fn troca_a_origem_nas_formulas() {
        let mut a = pasta_que_cita();
        a.escrever_celula(0, 5, 0, "='/home/x/[Vendas.fshz]Dados'!A1".into());
        a.instalar_externo_com("Vendas.fshz", vec![dados()], None, None);
        assert_eq!(a.trocar_origem_externa("vendas.fshz", "Vendas2026.fshz"), 2);
        let aba = &a.aba(0).unwrap().sheet;
        assert_eq!(aba.get_raw(0, 0), "=[Vendas2026.fshz]Dados!A1*2");
        assert_eq!(aba.get_raw(1, 0), "=SOMA([Vendas2026.fshz]Dados!B1:B3)");
        assert_eq!(aba.get_raw(5, 0), "='/home/x/[Vendas.fshz]Dados'!A1", "outro arquivo (com pasta) fica");
        // Os valores de antes continuam até chegar a nova origem.
        assert_eq!(a.valor(0, 0, 0), Value::Number(20.0));
        assert_eq!(a.trocar_origem_externa("Vendas2026.fshz", "/dados/Vendas.fshz"), 2);
        assert_eq!(a.aba(0).unwrap().sheet.get_raw(0, 0), "='/dados/[Vendas.fshz]Dados'!A1*2");
    }

    #[test]
    fn renomear_aba_daqui_nao_mexe_na_de_outro_arquivo() {
        assert_eq!(reescrever_aba("=[Vendas.fshz]Dados!A1+Dados!A1", "Dados", "X"), "=[Vendas.fshz]Dados!A1+X!A1");
        assert_eq!(reescrever_aba("='[Vendas.fshz]Dados'!A1", "Dados", "X"), "='[Vendas.fshz]Dados'!A1");
    }

    #[test]
    fn a_origem_entrega_os_valores_da_area() {
        let mut b = Workbook::new();
        b.renomear_aba(0, "Dados");
        b.escrever_celula(0, 0, 0, "4".into());
        b.escrever_celula(0, 1, 0, "=A1*2".into());
        assert_eq!(
            b.valores_da_area("dados", &Rect::new(0, 0, 9, 0)),
            Some(vec![(0, 0, Value::Number(4.0)), (1, 0, Value::Number(8.0))])
        );
        assert_eq!(b.valores_da_area("Nada", &Rect::new(0, 0, 0, 0)), None);
    }
}
