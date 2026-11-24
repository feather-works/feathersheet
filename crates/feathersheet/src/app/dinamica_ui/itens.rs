//! Itens de um campo (filtro, segmentação, páginas) quando a origem é grande: a coluna inteira é lida aos
//! poucos, [`ORCAMENTO`] por pulso, como a conta de fundo ([`super::fundo`]). Enquanto lê, vale o que já foi
//! lido, marcado como cortado. Ler 13,7 milhões de linhas de uma vez segurava a janela 0,66 s.
use super::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// Até quantas linhas a lista é lida na hora, inteira.
pub(super) const AMOSTRA: u32 = 20_000;
/// Itens distintos mostrados; passou disso, a lista é cortada.
pub(super) const MAX_ITENS: usize = 160;
/// Quanto a tela lê por pulso.
const ORCAMENTO: Duration = Duration::from_millis(25);

/// (id da aba de origem, versão dela, área, coluna).
type Chave = (u32, u32, [u32; 4], usize);

pub(super) struct Leitura {
    proxima: u32,
    pub(super) encontrados: BTreeMap<String, String>,
    pub(super) cortado: bool,
    pronto: bool,
}

impl Leitura {
    pub(super) fn nova(primeira: u32) -> Self {
        Self { proxima: primeira, encontrados: BTreeMap::new(), cortado: false, pronto: false }
    }
}

/// Lê de `l.proxima` até `ultima` (ou até `prazo`); para de vez ao passar de [`MAX_ITENS`].
pub(super) fn ler(app: &App, folha: usize, coluna: usize, ultima: u32, l: &mut Leitura, prazo: Option<Instant>) {
    let aba = app.workbook.aba(folha);
    while l.proxima <= ultima {
        if l.encontrados.len() >= MAX_ITENS {
            l.cortado = true;
            break;
        }
        // O relógio a cada 4 mil linhas: lê-lo por célula custaria mais que a leitura.
        if l.proxima % 4096 == 0 && prazo.is_some_and(|p| Instant::now() >= p) { return; }
        let r = l.proxima;
        l.proxima += 1;
        let valor = app.workbook.valor(folha, r, coluna as u32);
        let chave = if valor == Value::Empty { "(Vazio)".to_string() } else {
            let txt = to_text(&valor);
            if txt.trim().is_empty() { "(Vazio)".to_string() } else { txt }
        };
        if !l.encontrados.contains_key(&chave) {
            let estilo = aba.and_then(|a| a.sheet.estilo_resolvido(r, coluna as u32));
            // A chave fica como está (é o que o filtro salvo guarda); só o rótulo segue a língua.
            let rotulo = match &valor {
                _ if chave == "(Vazio)" => tr!("(Vazio)").to_owned(),
                Value::Bool(b) => feathersheet_core::celulas::locale::texto_logico(*b),
                Value::Error(e) => e.code.na_lingua(),
                _ => feathersheet_core::planilha::bloco::texto_formatado(&valor, estilo.as_ref()),
            };
            l.encontrados.insert(chave, rotulo);
        }
    }
    l.pronto = true;
}

/// As leituras em andamento e as prontas. `RefCell`: o pedido chega pela `view` (segmentação), que só tem `&App`.
#[derive(Default)]
pub(in crate::app) struct ItensFiltro {
    leituras: RefCell<HashMap<Chave, Leitura>>,
}

impl ItensFiltro {
    /// Alguma leitura ainda anda (liga o pulso).
    pub(in crate::app) fn lendo(&self) -> bool {
        self.leituras.borrow().values().any(|l| !l.pronto)
    }

    fn chave(app: &App, e: &Estado, coluna: usize) -> Option<Chave> {
        let a = app.workbook.aba(e.folha_origem)?;
        Some((a.id, a.versao, [e.origem.r0, e.origem.c0, e.origem.r1, e.origem.c1], coluna))
    }

    /// A lista desta coluna: pronta, ou o que já foi lido (cortada). O primeiro pedido lê a amostra na hora.
    pub(super) fn opcoes(&self, app: &App, e: &Estado, coluna: usize) -> (Vec<(String, String)>, bool) {
        let Some(chave) = Self::chave(app, e, coluna) else { return (Vec::new(), true) };
        let mut mapa = self.leituras.borrow_mut();
        if !mapa.contains_key(&chave) {
            // A mesma coluna de uma versão anterior da aba não vale mais.
            mapa.retain(|k, _| !(k.0 == chave.0 && k.2 == chave.2 && k.3 == chave.3));
            let mut l = Leitura::nova(e.origem.r0 + 1);
            ler(app, e.folha_origem, coluna, e.origem.r0 + AMOSTRA, &mut l, None);
            l.pronto = l.cortado;
            mapa.insert(chave, l);
        }
        let l = &mapa[&chave];
        (l.encontrados.iter().map(|(k, r)| (k.clone(), r.clone())).collect(), l.cortado || !l.pronto)
    }

    /// Esta lista ainda está sendo lida?
    pub(super) fn lendo_coluna(&self, app: &App, e: &Estado, coluna: usize) -> bool {
        Self::chave(app, e, coluna).is_some_and(|c| self.leituras.borrow().get(&c).is_some_and(|l| !l.pronto))
    }
}

impl App {
    /// Um pulso: lê mais um pedaço de cada lista em andamento. Terminou alguma: a segmentação e o filtro aberto
    /// mostram a lista inteira.
    pub(super) fn passo_itens_filtro(&mut self) {
        let prazo = Instant::now() + ORCAMENTO;
        let mut terminou = false;
        {
            let mut mapa = self.itens_filtro.leituras.borrow_mut();
            for (chave, l) in mapa.iter_mut().filter(|(_, l)| !l.pronto) {
                let Some(folha) = (0..self.workbook.len()).find(|&i| self.workbook.aba(i).is_some_and(|a| a.id == chave.0 && a.versao == chave.1)) else {
                    l.pronto = true;
                    continue;
                };
                ler(self, folha, chave.3, chave.2[2], l, Some(prazo));
                terminou |= l.pronto;
                if Instant::now() >= prazo { break; }
            }
        }
        if !terminou { return; }
        self.segmentacao.esquecer();
        let aberto = self.dinamica.as_ref().and_then(|e| e.campo_filtro.map(|c| (avancado::opcoes_de_filtro(self, e, c), c)));
        if let (Some(((itens, cortado), _)), Some(e)) = (aberto, self.dinamica.as_mut()) {
            e.valores_filtro = itens;
            e.filtro_cortado = cortado;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Item que só aparece depois das primeiras 20 mil linhas: entra quando a leitura aos poucos termina, e a
    /// lista só deixa de ser "cortada" aí.
    #[test]
    fn item_depois_da_amostra_aparece_quando_a_leitura_termina() {
        let mut app = App::default();
        let n = AMOSTRA + 5_000;
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Linha"); s.set_cell(0, 1, "Valor");
            for r in 1..=n { s.set_cell(r, 0, if r == n - 10 { "LOCOM" } else { "1" }); s.set_cell(r, 1, "2"); }
        });
        app.fim_selecao = (n, 1);
        app.evento_dinamica(Evento::Abrir);
        let e = app.dinamica.clone().expect("painel");
        let (antes, cortada) = avancado::opcoes_de_filtro(&app, &e, 0);
        assert!(cortada && antes.iter().all(|(k, _)| k != "LOCOM"), "{antes:?}");
        assert!(app.itens_filtro.lendo());
        let mut voltas = 0;
        while app.itens_filtro.lendo() && voltas < 1000 { app.passo_itens_filtro(); voltas += 1; }
        let (depois, cortada) = avancado::opcoes_de_filtro(&app, &e, 0);
        assert!(!cortada, "lida inteira");
        assert_eq!(depois.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["1", "LOCOM"]);
        // A aba mudou: a lista antiga não vale.
        app.workbook.editar(0, |s| s.set_cell(n, 0, "novo"));
        let (_, cortada) = avancado::opcoes_de_filtro(&app, &e, 0);
        assert!(cortada && app.itens_filtro.lendo(), "relê a versão nova");
    }
}
