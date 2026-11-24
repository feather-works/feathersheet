//! Temas da pasta (#80; referência: Layout da Página ▸ Temas do Excel): um tema tem 12 cores (claro/escuro 1 e 2,
//! seis ênfases, hiperlink e visitado, na ordem do `theme="n"` do Excel) e as fontes de título e de corpo. Fica em
//! `Workbook::tema` (JSON). As células e formas guardam a cor, não o índice do tema; então trocar o tema troca, em
//! todas as abas, cada cor do tema antigo (e os tons claros/escuros dela, os mesmos do seletor do Excel) pela cor
//! na mesma posição do tema novo, e a fonte de título/corpo pela nova. Um Ctrl+Z (foto da pasta).
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TemaPasta {
    pub nome: String,
    /// `0xRRGGBB` na ordem lt1, dk1, lt2, dk2, ênfase 1..6, hiperlink, visitado.
    pub cores: [u32; 12],
    pub titulo: String,
    pub corpo: String,
}

/// (nome, cores, fonte de título, fonte de corpo). O primeiro é o padrão (as cores do Office 2013+, que o `.xlsx`
/// sem `theme1.xml` usa).
const TEMAS: [(&str, [u32; 12], &str, &str); 6] = [
    ("FeatherSheet", [0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47, 0x0563C1, 0x954F72], "Padrão", "Padrão"),
    ("Oceano", [0xFFFFFF, 0x000000, 0xE8F1F8, 0x1F3B57, 0x0E6BA8, 0x0FA3B1, 0x4FC3F7, 0xF2A541, 0x2E86AB, 0x7BC67E, 0x0563C1, 0x5B4E9E], "Georgia", "Verdana"),
    ("Floresta", [0xFFFFFF, 0x000000, 0xEEF3E8, 0x2F3E2C, 0x2E7D32, 0x8BC34A, 0xA1887F, 0xFFB300, 0x558B2F, 0x6D4C41, 0x1B5E20, 0x795548], "Arial", "Arial"),
    ("Terracota", [0xFFFFFF, 0x000000, 0xF6EFE9, 0x4E342E, 0xC0532D, 0xE39B5A, 0x8D6E63, 0xD4A373, 0x6B705C, 0xA5A58D, 0xA0522D, 0x6D4C41], "Georgia", "Georgia"),
    ("Grafite", [0xFFFFFF, 0x000000, 0xEDEDED, 0x2B2B2B, 0x595959, 0x8C8C8C, 0x3F7CAC, 0xBF4342, 0xD9A441, 0x6A994E, 0x3F7CAC, 0x7B5EA7], "Verdana", "Verdana"),
    ("Lavanda", [0xFFFFFF, 0x000000, 0xF3EFF8, 0x3D2C5A, 0x7E57C2, 0xEC407A, 0x26A69A, 0xFFA726, 0x5C6BC0, 0x8D6E63, 0x5E35B1, 0xAD1457], "Times New Roman", "Arial"),
];

/// Os pares de fontes do menu Fontes: (título, corpo).
const FONTES: [(&str, &str); 6] = [
    ("Padrão", "Padrão"), ("Georgia", "Verdana"), ("Arial", "Arial"), ("Times New Roman", "Times New Roman"),
    ("Verdana", "Verdana"), ("Georgia", "Georgia"),
];

/// Os tons do seletor do Excel (mais claro 80/60/40%, mais escuro 25/50%).
const TONS: [f64; 5] = [0.8, 0.6, 0.4, -0.25, -0.5];

impl TemaPasta {
    fn pronto(i: usize) -> Self {
        let (nome, cores, titulo, corpo) = TEMAS[i.min(TEMAS.len() - 1)];
        TemaPasta { nome: nome.into(), cores, titulo: titulo.into(), corpo: corpo.into() }
    }
    pub(crate) fn padrao() -> Self { Self::pronto(0) }
}

/// O tom `t` (−1..1) da cor: o mesmo `tint` que a leitura do `.xlsx` aplica, para as cores com tom de um
/// arquivo do Excel serem achadas na troca de tema.
fn tom(cor: u32, t: f64) -> u32 {
    feathersheet_io::xlsx_read::cores::aplicar_tint(cor, t)
}

fn hex(c: u32) -> String { format!("#{c:06x}") }

/// Cada cor do tema antigo (e os tons dela) → a da mesma posição no novo. Chave em minúsculas.
fn mapa_de_cores(antigo: &TemaPasta, novo: &TemaPasta) -> HashMap<String, String> {
    let mut mapa = HashMap::new();
    for (a, n) in antigo.cores.iter().zip(&novo.cores) {
        let mut par = |x: u32, y: u32| { if x != y { mapa.entry(hex(x)).or_insert_with(|| hex(y)); } };
        par(*a, *n);
        for t in TONS { par(tom(*a, t), tom(*n, t)); }
    }
    mapa
}

#[derive(Debug, Clone)]
pub enum Evento {
    /// Um tema pronto inteiro (cores e fontes).
    Tema(usize),
    /// Só as cores de um tema pronto.
    Cores(usize),
    /// Só o par de fontes.
    Fontes(usize),
}

impl App {
    /// O tema da pasta (o padrão, se ela não tem).
    pub(crate) fn tema_da_pasta(&self) -> TemaPasta {
        self.workbook.tema.as_deref().and_then(|t| serde_json::from_str(t).ok()).unwrap_or_else(TemaPasta::padrao)
    }

    pub(super) fn evento_tema_pasta(&mut self, e: Evento) {
        self.edicao_menu = None;
        let atual = self.tema_da_pasta();
        let novo = match e {
            Evento::Tema(i) => TemaPasta::pronto(i),
            Evento::Cores(i) => TemaPasta { cores: TemaPasta::pronto(i).cores, ..atual.clone() },
            Evento::Fontes(i) => {
                let (titulo, corpo) = FONTES[i.min(FONTES.len() - 1)];
                TemaPasta { titulo: titulo.into(), corpo: corpo.into(), ..atual.clone() }
            }
        };
        self.aplicar_tema_pasta(&atual, novo);
    }

    /// Troca o tema: recolore células e formas de todas as abas e grava o tema na pasta. Um Ctrl+Z.
    fn aplicar_tema_pasta(&mut self, antigo: &TemaPasta, novo: TemaPasta) {
        if *antigo == novo { return; }
        let foto = self.foto_antes_da_estrutura();
        let cores = mapa_de_cores(antigo, &novo);
        let cor = |c: &str| cores.get(&c.to_ascii_lowercase()).cloned();
        let fonte = |f: &str| if f == antigo.titulo && antigo.titulo != novo.titulo { Some(novo.titulo.clone()) }
            else if f == antigo.corpo && antigo.corpo != novo.corpo { Some(novo.corpo.clone()) } else { None };
        let mut mudaram = 0;
        for i in 0..self.workbook.len() {
            self.workbook.editar(i, |s| { mudaram += s.remapear_estilos(&cor, &fonte); });
            let Some(aba) = self.workbook.aba_mut(i) else { continue };
            let Ok(mut objetos) = feathersheet_io::objetos_aba::ObjetosAba::ler(aba.sheet.ob.as_deref()) else { continue };
            let mut formas = false;
            for o in &mut objetos.objetos {
                let mut trocar = |v: &mut Option<String>| if let Some(n) = v.as_deref().and_then(cor) { *v = Some(n); formas = true; };
                trocar(&mut o.aparencia.preenchimento);
                trocar(&mut o.aparencia.contorno);
                if let Some(f) = o.fonte.as_mut() {
                    trocar(&mut f.cor);
                    if let Some(n) = f.familia.as_deref().and_then(fonte) { f.familia = Some(n); formas = true; }
                }
            }
            if formas { aba.sheet.ob = objetos.gravar(); mudaram += 1; }
        }
        self.workbook.tema = serde_json::to_string(&novo).ok();
        self.alterado = true;
        self.guardar_foto_da_estrutura(foto, true);
        self.status = trf!("Tema {}: {} estilo(s) e forma(s) recoloridos.", novo.nome, mudaram);
    }

    /// Os itens dos menus Temas, Cores e Fontes (o atual com a marca).
    pub(super) fn itens_menu_tema(&self, tipo: &str) -> Vec<(String, &'static str, Msg)> {
        let atual = self.tema_da_pasta();
        let marca = |sim: bool, icone: &'static str| if sim { "check" } else { icone };
        match tipo {
            "tm_temas" => TEMAS.iter().enumerate().map(|(i, (nome, cores, t, c))| (format!("{nome} · {t} / {c}"),
                marca(atual.nome == *nome && atual.cores == *cores && atual.titulo == *t && atual.corpo == *c, "palette"),
                Msg::TemaPasta(Evento::Tema(i)))).collect(),
            "tm_cores" => TEMAS.iter().enumerate().map(|(i, (nome, cores, ..))| (nome.to_string(),
                marca(atual.cores == *cores, "palette"), Msg::TemaPasta(Evento::Cores(i)))).collect(),
            "tm_fontes" => FONTES.iter().enumerate().map(|(i, (t, c))| (format!("{} / {}", tr!(t), tr!(c)),
                marca(atual.titulo == *t && atual.corpo == *c, "text-size"), Msg::TemaPasta(Evento::Fontes(i)))).collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::estilo_parcial::Campo;

    #[test]
    fn tons_como_o_excel() {
        // Ênfase 1 do Office (#4472C4): 80% mais clara = #DAE3F3, 25% mais escura = #2F5597 (o seletor do Excel).
        assert_eq!(hex(tom(0x4472C4, 0.8)), "#dae3f3");
        assert_eq!(hex(tom(0x4472C4, -0.25)), "#2f5597");
        assert_eq!(tom(0xFFFFFF, 0.5), 0xFFFFFF);
        assert_eq!(tom(0x000000, -0.5), 0x000000);
    }

    /// Trocar o tema: a ênfase 1 e um tom dela viram os do tema novo, uma cor que não é do tema fica, a fonte de
    /// corpo troca; a forma também; o tema fica na pasta e um Ctrl+Z volta tudo.
    #[test]
    fn trocar_o_tema_recolore_celulas_e_formas() {
        let mut app = App::default();
        let floresta = TemaPasta::pronto(2);
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "x");
            s.set_style(&Rect::new(0, 0, 0, 0), &[("bg".to_string(), Campo::Texto("#4472C4".into())),
                ("color".to_string(), Campo::Texto(hex(tom(0x4472C4, 0.8)))), ("fontFamily".to_string(), Campo::Texto("Padrão".into()))].into());
            s.set_cell(1, 0, "y");
            s.set_style(&Rect::new(1, 0, 1, 0), &[("bg".to_string(), Campo::Texto("#123456".into()))].into());
        });
        let mut objetos = feathersheet_io::objetos_aba::ObjetosAba::default();
        let regua = super::super::objetos::ReguaAba(&app.workbook.aba(0).unwrap().sheet);
        let mut forma = feathersheet_io::objetos_aba::ObjetoSolto::nova_forma(1, "retangulo",
            feathersheet_io::objetos_aba::Retangulo::novo(10.0, 10.0, 50.0, 50.0), &regua);
        forma.aparencia.preenchimento = Some("#ed7d31".into());
        objetos.adicionar(forma);
        app.workbook.aba_mut(0).unwrap().sheet.ob = objetos.gravar();

        app.evento_tema_pasta(Evento::Tema(2));
        let s = &app.workbook.aba(0).unwrap().sheet;
        let a1 = s.get_style(0, 0).unwrap();
        assert_eq!(a1.bg.as_deref(), Some(hex(floresta.cores[4]).as_str()));
        assert_eq!(a1.color.as_deref(), Some(hex(tom(floresta.cores[4], 0.8)).as_str()), "o tom acompanha");
        assert_eq!(a1.font_family.as_deref(), Some("Arial"));
        assert_eq!(s.get_style(1, 0).unwrap().bg.as_deref(), Some("#123456"), "cor de fora do tema fica");
        let o = feathersheet_io::objetos_aba::ObjetosAba::ler(s.ob.as_deref()).unwrap();
        assert_eq!(o.objetos[0].aparencia.preenchimento.as_deref(), Some(hex(floresta.cores[5]).as_str()));
        assert_eq!(app.tema_da_pasta(), floresta);
        assert!(app.itens_menu_tema("tm_temas")[2].1 == "check");

        // Só as cores (Oceano) mantêm as fontes da Floresta; só as fontes mantêm as cores.
        app.evento_tema_pasta(Evento::Cores(1));
        assert_eq!(app.tema_da_pasta().corpo, "Arial");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_style(0, 0).unwrap().bg.as_deref(), Some(hex(TemaPasta::pronto(1).cores[4]).as_str()));
        app.evento_tema_pasta(Evento::Fontes(3));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_style(0, 0).unwrap().font_family.as_deref(), Some("Times New Roman"));

        // Ctrl+Z volta a pasta de antes da última troca.
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.tema_da_pasta().corpo, "Arial");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_style(0, 0).unwrap().font_family.as_deref(), Some("Arial"));
        for t in ["tm_temas", "tm_cores", "tm_fontes"] { assert!(!app.itens_menu_tema(t).is_empty()); }
        for i in ["check", "palette", "text-size", "brush"] { assert!(feather_ui::icones::existe(i), "{i}"); }
    }
}
