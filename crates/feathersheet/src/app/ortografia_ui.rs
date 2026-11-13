//! Revisão ▸ Verificar Ortografia (F7, #82), como no Excel: percorre o texto digitado das
//! células (não as fórmulas) a partir da ativa, para em cada palavra fora do dicionário e
//! oferece Ignorar, Ignorar tudo, Adicionar ao dicionário, Alterar e Alterar todas.
//! Com várias células selecionadas, só confere a seleção.
//!
//! O dicionário é a lista de palavras do sistema (uma palavra por linha) mais o dicionário
//! pessoal (`dicionario-pessoal.txt`, na pasta de preferências).
// shortcut: as listas vêm de /usr/share/dict (pacotes wbrazilian, wamerican, wspanish, wfrench do
// Linux); no Windows e no macOS só vale o arquivo `dicionario-<idioma>.txt` posto na pasta de
// preferências. Embutir listas no app se pedirem.
use super::*;
use crate::idioma::Lingua;
use feathersheet_core::celulas::refs::cell_name;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

const LINGUAS: [Lingua; 4] = [Lingua::Pt, Lingua::En, Lingua::Es, Lingua::Fr];

fn nome_lingua(l: Lingua) -> &'static str {
    match l { Lingua::Pt => "Português", Lingua::En => "English", Lingua::Es => "Español", Lingua::Fr => "Français" }
}

fn listas_do_sistema(l: Lingua) -> &'static [&'static str] {
    match l {
        Lingua::Pt => &["/usr/share/dict/brazilian", "/usr/share/dict/portuguese"],
        Lingua::En => &["/usr/share/dict/american-english", "/usr/share/dict/british-english", "/usr/share/dict/words"],
        Lingua::Es => &["/usr/share/dict/spanish"],
        Lingua::Fr => &["/usr/share/dict/french"],
    }
}

/// Pasta das preferências do FeatherSheet (a mesma do `config-iced.json`).
fn pasta() -> Option<PathBuf> {
    crate::configuracao::caminho_config().and_then(|p| p.parent().map(PathBuf::from))
}

fn pessoal() -> Option<PathBuf> { pasta().map(|p| p.join("dicionario-pessoal.txt")) }

fn juntar(conjunto: &mut HashSet<String>, texto: &str) {
    conjunto.extend(texto.lines().map(str::trim).filter(|p| !p.is_empty()).map(str::to_lowercase));
}

fn cache() -> &'static Mutex<HashMap<&'static str, Arc<HashSet<String>>>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, Arc<HashSet<String>>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// O dicionário do idioma (lido uma vez por sessão), ou `None` sem lista nenhuma.
fn dicionario(l: Lingua) -> Option<Arc<HashSet<String>>> {
    let mut cache = cache().lock().ok()?;
    if let Some(d) = cache.get(l.codigo()) { return Some(d.clone()); }
    let mut palavras = HashSet::new();
    let mut achou = false;
    let proprio = pasta().map(|p| p.join(format!("dicionario-{}.txt", l.codigo())));
    for caminho in listas_do_sistema(l).iter().map(PathBuf::from).chain(proprio) {
        if let Ok(texto) = std::fs::read_to_string(&caminho) { juntar(&mut palavras, &texto); achou = true; }
    }
    if !achou { return None; }
    if let Some(Ok(texto)) = pessoal().map(std::fs::read_to_string) { juntar(&mut palavras, &texto); }
    let d = Arc::new(palavras);
    cache.insert(l.codigo(), d.clone());
    Some(d)
}

/// As palavras conferidas de um texto: (início em bytes, palavra). Fica de fora o que tem
/// algarismo, e-mail e endereço web, sigla em maiúsculas e letra solta (como no Excel).
pub(super) fn palavras(texto: &str) -> Vec<(usize, &str)> {
    let mut saida = Vec::new();
    let mut pos = 0;
    for pedaco in texto.split_inclusive(char::is_whitespace) {
        let token = pedaco.trim_end();
        let pular = token.contains(|c: char| c.is_ascii_digit()) || token.contains('@')
            || token.contains("://") || token.to_lowercase().starts_with("www.");
        if !pular {
            let mut inicio = None;
            for (i, c) in token.char_indices().chain(std::iter::once((token.len(), ' '))) {
                match (c.is_alphabetic(), inicio) {
                    (true, None) => inicio = Some(i),
                    (false, Some(a)) => {
                        let p = &token[a..i];
                        let sigla = p.chars().all(|c| !c.is_lowercase());
                        if p.chars().count() > 1 && !sigla { saida.push((pos + a, p)); }
                        inicio = None;
                    }
                    _ => {}
                }
            }
        }
        pos += pedaco.len();
    }
    saida
}

fn conhecida(dic: &HashSet<String>, ignoradas: &HashSet<String>, p: &str) -> bool {
    let m = p.to_lowercase();
    dic.contains(&m) || ignoradas.contains(&m)
}

/// Distância de edição (com troca de vizinhas) até `limite`; acima dele devolve `limite + 1`.
fn distancia(a: &[char], b: &[char], limite: usize) -> usize {
    if a.len().abs_diff(b.len()) > limite { return limite + 1; }
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, linha) in d.iter_mut().enumerate() { linha[0] = i; }
    for j in 0..=b.len() { d[0][j] = j; }
    for i in 1..=a.len() {
        let mut menor = usize::MAX;
        for j in 1..=b.len() {
            let custo = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + custo);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] { v = v.min(d[i - 2][j - 2] + 1); }
            d[i][j] = v;
            menor = menor.min(v);
        }
        if menor > limite { return limite + 1; }
    }
    d[a.len()][b.len()]
}

/// Até 6 sugestões, as mais próximas primeiro, com as maiúsculas da palavra digitada.
pub(super) fn sugestoes(dic: &HashSet<String>, palavra: &str) -> Vec<String> {
    let alvo: Vec<char> = palavra.to_lowercase().chars().collect();
    let limite = if alvo.len() <= 4 { 1 } else { 2 };
    let mut achadas: Vec<(usize, bool, &String)> = dic.iter().filter_map(|w| {
        let c: Vec<char> = w.chars().collect();
        let d = distancia(&alvo, &c, limite);
        (d <= limite).then(|| (d, c.first() != alvo.first(), w))
    }).collect();
    achadas.sort();
    let maiuscula = palavra.chars().next().is_some_and(char::is_uppercase);
    achadas.into_iter().take(6).map(|(_, _, w)| {
        if !maiuscula { return w.clone(); }
        let mut c = w.chars();
        c.next().map(|p| p.to_uppercase().chain(c).collect()).unwrap_or_default()
    }).collect()
}

/// Troca as ocorrências da palavra inteira (mesmas maiúsculas) no texto.
fn trocar_inteira(texto: &str, de: &str, para: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut feito = 0;
    for (i, p) in palavras(texto) {
        if p == de {
            saida.push_str(&texto[feito..i]);
            saida.push_str(para);
            feito = i + p.len();
        }
    }
    saida.push_str(&texto[feito..]);
    saida
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Fechar,
    Lingua(Lingua),
    Troca(String),
    Sugestao(String),
    Ignorar,
    IgnorarTudo,
    Adicionar,
    Alterar,
    AlterarTodas,
}

#[derive(Debug, Clone)]
struct Achado { linha: u32, coluna: u32, inicio: usize, palavra: String, texto: String }

#[derive(Debug, Clone)]
pub(super) struct Estado {
    lingua: Lingua,
    dic: Arc<HashSet<String>>,
    ignoradas: HashSet<String>,
    area: Rect,
    /// Células da área já vistas, contando da primeira (a ativa): para ao dar a volta.
    vistas: u64,
    /// Onde continuar: (linha, coluna, byte dentro da célula).
    pos: (u32, u32, usize),
    achado: Option<Achado>,
    troca: String,
    sugestoes: Vec<String>,
}

impl Estado {
    /// A próxima palavra desconhecida a partir de `pos`, dando a volta na área uma vez.
    fn procurar(&mut self, sheet: &feathersheet_core::planilha::sheet::Sheet) {
        let a = &self.area;
        let total = u64::from(a.r1 - a.r0 + 1) * u64::from(a.c1 - a.c0 + 1);
        let (mut r, mut c, mut byte) = self.pos;
        while self.vistas < total {
            let bruto = sheet.get_raw(r, c);
            if !bruto.starts_with('=') {
                if let Some(&(i, p)) = palavras(&bruto).iter()
                    .find(|(i, p)| *i >= byte && !conhecida(&self.dic, &self.ignoradas, p)) {
                    self.pos = (r, c, i);
                    self.sugestoes = sugestoes(&self.dic, p);
                    self.troca = self.sugestoes.first().cloned().unwrap_or_else(|| p.to_owned());
                    self.achado = Some(Achado { linha: r, coluna: c, inicio: i, palavra: p.to_owned(), texto: bruto.clone() });
                    return;
                }
            }
            byte = 0;
            self.vistas += 1;
            if c < a.c1 { c += 1 } else { c = a.c0; r = if r < a.r1 { r + 1 } else { a.r0 }; }
        }
        self.achado = None;
    }
}

impl App {
    pub(super) fn evento_ortografia(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                if !self.confirmar() { return Task::none(); }
                return self.comecar_ortografia(crate::idioma::lingua());
            }
            Evento::Fechar => self.ortografia = None,
            Evento::Lingua(l) => return self.comecar_ortografia(l),
            Evento::Troca(t) | Evento::Sugestao(t) => if let Some(e) = &mut self.ortografia { e.troca = t; },
            Evento::Ignorar => {
                if let Some(e) = &mut self.ortografia {
                    if let Some(a) = &e.achado { e.pos.2 = a.inicio + a.palavra.len(); }
                }
                self.seguir_ortografia();
            }
            Evento::IgnorarTudo | Evento::Adicionar => {
                let Some(e) = &mut self.ortografia else { return Task::none() };
                let Some(a) = e.achado.clone() else { return Task::none() };
                e.ignoradas.insert(a.palavra.to_lowercase());
                if matches!(evento, Evento::Adicionar) {
                    if let Err(erro) = adicionar_ao_pessoal(&a.palavra) {
                        self.status = trf!("Não foi possível gravar o dicionário pessoal: {erro}");
                    }
                }
                self.seguir_ortografia();
            }
            Evento::Alterar | Evento::AlterarTodas => {
                let Some(e) = self.ortografia.clone() else { return Task::none() };
                let Some(a) = e.achado else { return Task::none() };
                let troca = e.troca.trim().to_owned();
                if troca.is_empty() { return Task::none(); }
                self.liberar_areas();
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                // Alterar: só esta célula. Alterar todas: toda célula da área com a palavra.
                let alvos: Vec<(u32, u32, String)> = if matches!(evento, Evento::Alterar) {
                    let mut novo = a.texto.clone();
                    novo.replace_range(a.inicio..a.inicio + a.palavra.len(), &troca);
                    vec![(a.linha, a.coluna, novo)]
                } else {
                    let r = &e.area;
                    (r.r0..=r.r1).flat_map(|l| (r.c0..=r.c1).map(move |c| (l, c))).filter_map(|(l, c)| {
                        let bruto = aba.sheet.get_raw(l, c);
                        if bruto.starts_with('=') { return None; }
                        let novo = trocar_inteira(&bruto, &a.palavra, &troca);
                        (novo != bruto).then_some((l, c, novo))
                    }).collect()
                };
                if alvos.iter().any(|(l, c, _)| aba.sheet.celula_bloqueada(*l, *c)) {
                    self.status = tr!("A célula está protegida: desproteja a planilha para corrigir.").into();
                    if let Some(e) = &mut self.ortografia { e.pos.2 = a.inicio + a.palavra.len(); }
                    self.seguir_ortografia();
                    return Task::none();
                }
                self.workbook.editar(self.folha, |s| {
                    s.abrir_transacao();
                    for (l, c, novo) in &alvos { s.set_cell(*l, *c, novo); }
                    s.fechar_transacao();
                });
                self.alterado = true;
                if let Some(e) = &mut self.ortografia {
                    // A célula mudou de tamanho: continuar depois da palavra trocada.
                    e.pos.2 = a.inicio + troca.len();
                }
                self.seguir_ortografia();
            }
        }
        Task::none()
    }

    fn comecar_ortografia(&mut self, lingua: Lingua) -> Task<Msg> {
        let Some(dic) = dicionario(lingua) else {
            self.ortografia = None;
            self.status = trf!("Sem dicionário de {}: instale a lista de palavras do sistema ou ponha uma em {}.",
                nome_lingua(lingua),
                pasta().map(|p| p.join(format!("dicionario-{}.txt", lingua.codigo())).display().to_string()).unwrap_or_default());
            return Task::none();
        };
        let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
        let sel = self.area_selecionada();
        let varias = sel.r0 != sel.r1 || sel.c0 != sel.c1;
        let area = if varias { sel } else { aba.sheet.used_rect() };
        let (r, c) = if area.contains(self.linha, self.coluna) { (self.linha, self.coluna) } else { (area.r0, area.c0) };
        let ignoradas = self.ortografia.take().map(|e| e.ignoradas).unwrap_or_default();
        self.ortografia = Some(Estado {
            lingua, dic, ignoradas, area, vistas: 0, pos: (r, c, 0),
            achado: None, troca: String::new(), sugestoes: Vec::new(),
        });
        self.seguir_ortografia();
        Task::none()
    }

    /// Procura a próxima palavra; sem nenhuma, fecha e avisa como o Excel.
    fn seguir_ortografia(&mut self) {
        let Some(mut e) = self.ortografia.take() else { return };
        if let Some(aba) = self.workbook.aba(self.folha) { e.procurar(&aba.sheet); }
        match &e.achado {
            Some(a) => {
                let (l, c) = (a.linha, a.coluna);
                self.ortografia = Some(e);
                self.trocar_selecao(l, c);
            }
            None => self.status = tr!("A verificação ortográfica foi concluída.").into(),
        }
    }

    fn view_ortografia(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.ortografia.as_ref().expect("ortografia aberta");
        let a = e.achado.as_ref().expect("palavra achada");
        let m = |ev: Evento| Msg::Ortografia(ev);
        let rotulo = |t: &str| text(t.to_owned()).size(12).color(COR_TEXTO_MUTED());
        // O trecho em volta da palavra, com a palavra entre aspas.
        let antes: String = a.texto[..a.inicio].chars().rev().take(60).collect::<Vec<_>>().into_iter().rev().collect();
        let depois: String = a.texto[a.inicio + a.palavra.len()..].chars().take(60).collect();
        let trecho = format!("{antes}“{}”{depois}", a.palavra);
        let mut lista = pelicano::column(Vec::<Element<'_, Msg>>::new()).spacing(2);
        for s in &e.sugestoes {
            lista = lista.push(button(text(s.clone()).size(13)).width(Length::FILL).padding([4, 8])
                .style(estilo_botao_pequeno(*s == e.troca)).on_press(m(Evento::Sugestao(s.clone()))));
        }
        if e.sugestoes.is_empty() { lista = lista.push(rotulo(tr!("(sem sugestões)"))); }
        let botao = |t: &str, ev: Evento| button(text(t.to_owned()).size(12)).style(estilo_botao_secundario)
            .padding([6, 10]).on_press(m(ev));
        let corpo = pelicano::column![
            row![
                rotulo(tr!("Não consta no dicionário")),
                Space::new(Length::FILL, 0.0),
                text(cell_name(a.linha, a.coluna)).size(12).font(pelicano::Font::MONO),
            ].align(pelicano::Align::Center),
            container(text(trecho).size(13)).padding(8).width(Length::FILL).style(estilo_painel_faixa),
            rotulo(tr!("Alterar para")),
            text_input("", &e.troca).on_input(move |t| m(Evento::Troca(t))).on_submit(m(Evento::Alterar)).padding(8).size(13),
            rotulo(tr!("Sugestões")),
            lista,
            row![
                botao(tr!("Ignorar uma vez"), Evento::Ignorar),
                botao(tr!("Ignorar tudo"), Evento::IgnorarTudo),
                botao(tr!("Adicionar ao dicionário"), Evento::Adicionar),
            ].spacing(6),
            row![
                rotulo(tr!("Idioma do dicionário")),
                pelicano::pick_list(&LINGUAS[..], Some(e.lingua), move |l| m(Evento::Lingua(l))).size(12.5),
            ].spacing(8).align(pelicano::Align::Center),
        ].spacing(8);
        self.dialogo(tr!("Ortografia"), corpo)
            .icone("checks")
            .largura(460.0)
            .ao_fechar(m(Evento::Fechar))
            .secundario(tr!("Alterar todas"), Some(m(Evento::AlterarTodas)))
            .primario(tr!("Alterar"), Some(m(Evento::Alterar)))
    }

    pub(super) fn sobrepor_ortografia<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.ortografia.as_ref().is_some_and(|e| e.achado.is_some()) { self.view_ortografia().sobre(base) } else { base }
    }
}

impl std::fmt::Display for Lingua {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(nome_lingua(*self)) }
}

fn adicionar_ao_pessoal(palavra: &str) -> std::io::Result<()> {
    use std::io::Write;
    let caminho = pessoal().ok_or_else(|| std::io::Error::other("sem pasta de preferências"))?;
    if let Some(p) = caminho.parent() { std::fs::create_dir_all(p)?; }
    writeln!(std::fs::OpenOptions::new().create(true).append(true).open(&caminho)?, "{palavra}")?;
    // A próxima abertura relê o pessoal junto com a lista do sistema.
    if let Ok(mut c) = cache().lock() { c.clear(); }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn separa_palavras_e_pula_numeros_siglas_e_enderecos() {
        let p: Vec<&str> = palavras("Olá, mundo! 10kg ONU a teste@x.com www.site.com d'água").into_iter().map(|(_, p)| p).collect();
        assert_eq!(p, ["Olá", "mundo", "água"]);
        assert_eq!(palavras("um  dois")[1], (4, "dois"));
    }

    #[test]
    fn sugere_a_mais_proxima_com_a_maiuscula_digitada() {
        let dic: HashSet<String> = ["você", "casa", "cada", "planilha", "plano"].map(String::from).into();
        assert_eq!(sugestoes(&dic, "voce")[0], "você");
        assert_eq!(sugestoes(&dic, "Planliha")[0], "Planilha", "troca de vizinhas conta 1");
        assert!(sugestoes(&dic, "xyzw").is_empty());
        assert_eq!(trocar_inteira("casaa e casaa, casaal", "casaa", "casa"), "casa e casa, casaal");
    }

    #[test]
    fn para_em_cada_palavra_desconhecida_e_altera_a_celula() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "uma casaa bonita".into());
        app.workbook.escrever_celula(0, 1, 0, "=SOMA(casaa)".into());
        app.workbook.escrever_celula(0, 2, 0, "outra casaa".into());
        let dic: HashSet<String> = ["uma", "casa", "bonita", "outra"].map(String::from).into();
        app.ortografia = Some(Estado {
            lingua: Lingua::Pt, dic: Arc::new(dic), ignoradas: HashSet::new(),
            area: app.workbook.aba(0).unwrap().sheet.used_rect(), vistas: 0, pos: (0, 0, 0),
            achado: None, troca: String::new(), sugestoes: Vec::new(),
        });
        app.seguir_ortografia();
        let a = app.ortografia.as_ref().unwrap().achado.clone().unwrap();
        assert_eq!((a.linha, a.palavra.as_str()), (0, "casaa"));
        assert_eq!(app.ortografia.as_ref().unwrap().troca, "casa");
        let _ = app.update(Msg::Ortografia(Evento::Alterar));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "uma casa bonita");
        // A fórmula fica de fora; a próxima é a da linha 3.
        assert_eq!(app.ortografia.as_ref().unwrap().achado.as_ref().unwrap().linha, 2);
        let _ = app.update(Msg::Ortografia(Evento::IgnorarTudo));
        assert!(app.ortografia.is_none(), "deu a volta: concluída");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 0), "outra casaa");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "uma casaa bonita");
    }
}
