//! O desenho dos formulários (UserForm) de um projeto VBA: cada um é uma pasta do
//! `vbaProject.bin` com o nome do módulo e os fluxos `f` (o formulário e a lista dos
//! controles — os "sites") e `o` (os dados de cada controle, um depois do outro), no formato
//! do [MS-OFORMS]. Os controles de uma moldura (`Frame`) moram numa subpasta `i<ID>` com os
//! seus próprios `f` e `o`.
//!
//! Lido: título, tamanho e fundo do formulário; de cada controle, tipo, nome, posição,
//! tamanho, texto, valor, cores, letra, `RowSource`, `MaxLength`, `PasswordChar`,
//! `MultiSelect`, `MultiLine`, `Enabled`, `Visible`, `Default`/`Cancel`, dica e figura.
//! O que o formato traz e o FeatherSheet não tem (`SpinButton`, `ScrollBar`, `TabStrip`,
//! `MultiPage`) vira [`TipoVba::Outro`], e quem converte avisa.

/// Um formulário lido, em px (96 dpi).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormularioVba {
    pub nome: String,
    pub titulo: String,
    pub largura: f32,
    pub altura: f32,
    /// `#rrggbb`; vazio = cor do sistema (segue o tema).
    pub fundo: String,
    pub controles: Vec<ControleVba>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TipoVba {
    #[default]
    Rotulo,
    Texto,
    /// `ComboBox`: lista que aceita digitar (`livre`), salvo `DropList`.
    Lista,
    /// `ListBox`.
    Listagem,
    Caixa,
    Opcao,
    /// `ToggleButton`: vira caixa de seleção.
    Alternar,
    Botao,
    Moldura,
    Imagem,
    /// `ClsidCacheIndex` sem equivalente.
    Outro(u16),
}

/// Um controle, com posição relativa ao formulário (já somada a da moldura).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControleVba {
    pub tipo: TipoVba,
    pub nome: String,
    pub texto: String,
    pub valor: String,
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
    pub fundo: String,
    pub cor: String,
    pub fonte: String,
    pub max: u32,
    pub senha: bool,
    pub multipla: bool,
    pub multilinha: bool,
    /// ComboBox em que só se escolhe da lista (`DropList`).
    pub so_lista: bool,
    pub desativado: bool,
    pub oculto: bool,
    pub padrao: bool,
    pub cancelar: bool,
    pub dica: String,
    /// `GroupName` das opções (vazio = o grupo é o formulário ou a moldura).
    pub grupo: String,
    /// Tamanho da letra em pontos.
    pub letra: Option<f32>,
    pub negrito: bool,
    pub italico: bool,
    /// "centro" ou "direita" (vazio = esquerda).
    pub alinhar: String,
    /// Bytes da figura (`Image.Picture`), como estavam no arquivo.
    pub figura: Vec<u8>,
    /// A ordem do Tab (`TabIndex`).
    pub ordem: u16,
    /// Itens da lista (do `AddItem` com texto fixo no `UserForm_Initialize`).
    pub itens: Vec<String>,
}

/// Tetos para um arquivo estranho não pedir memória sem fim.
const MAX_CONTROLES: usize = 500;
const MAX_PROFUNDIDADE: usize = 4;

/// HIMETRIC (centésimos de milímetro) → px a 96 dpi.
fn px(himetric: i32) -> f32 {
    (himetric as f32 * 96.0 / 2540.0).round()
}

/// `OLE_COLOR`: `0x00BBGGRR`; com o bit alto é cor do sistema (vazio: segue o tema).
fn cor(c: u32) -> String {
    if c & 0xFF00_0000 != 0 {
        return String::new();
    }
    format!("#{:02x}{:02x}{:02x}", c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF)
}

/// Um leitor de bytes que nunca entra em pânico: fora do fim, `None`.
struct Leitor<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Leitor<'a> {
    fn novo(d: &'a [u8]) -> Self {
        Self { d, p: 0 }
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.d.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(b)
    }
    fn u8(&mut self) -> Option<u8> {
        self.bytes(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.bytes(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.bytes(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }
    /// Alinha a posição a `n` bytes, contados desde `base`.
    fn alinhar(&mut self, base: usize, n: usize) {
        let resto = (self.p.saturating_sub(base)) % n;
        if resto != 0 {
            self.p += n - resto;
        }
    }
}

/// Um bloco `DataBlock` + `ExtraDataBlock` do MS-OFORMS: cada campo do primeiro alinhado ao
/// próprio tamanho, os textos e tamanhos do segundo alinhados a 4.
struct Bloco<'a, 'b> {
    l: &'b mut Leitor<'a>,
    base: usize,
    mascara: u64,
}

impl<'a> Bloco<'a, '_> {
    fn tem(&self, bit: u32) -> bool {
        self.mascara & (1 << bit) != 0
    }
    fn u8(&mut self, bit: u32) -> Option<u8> {
        if !self.tem(bit) {
            return None;
        }
        self.l.u8()
    }
    fn u16(&mut self, bit: u32) -> Option<u16> {
        if !self.tem(bit) {
            return None;
        }
        self.l.alinhar(self.base, 2);
        self.l.u16()
    }
    fn u32(&mut self, bit: u32) -> Option<u32> {
        if !self.tem(bit) {
            return None;
        }
        self.l.alinhar(self.base, 4);
        self.l.u32()
    }
    /// O tamanho de um texto (`CountOfBytesWithCompressionFlag`): (bytes, comprimido).
    fn tamanho(&mut self, bit: u32) -> Option<(usize, bool)> {
        self.u32(bit).map(|v| ((v & 0x7FFF_FFFF) as usize, v & 0x8000_0000 != 0))
    }
    /// Fim do `DataBlock`: alinha a 4.
    fn fim_dos_dados(&mut self) {
        self.l.alinhar(self.base, 4);
    }
    /// Um texto do `ExtraDataBlock`.
    fn texto(&mut self, t: Option<(usize, bool)>) -> String {
        let Some((n, comprimido)) = t else { return String::new() };
        let Some(b) = self.l.bytes(n) else { return String::new() };
        self.l.alinhar(self.base, 4);
        if comprimido {
            b.iter().map(|&c| char::from(c)).collect()
        } else {
            let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&u)
        }
    }
    /// Um par (largura, altura) ou (esquerda, topo) do `ExtraDataBlock`.
    fn par(&mut self, bit: u32) -> Option<(i32, i32)> {
        if !self.tem(bit) {
            return None;
        }
        self.l.alinhar(self.base, 4);
        Some((self.l.i32()?, self.l.i32()?))
    }
}

/// Abre um bloco de controle: versão, `cb` e a máscara (de 4 ou 8 bytes). Devolve o bloco e
/// onde ele acaba (`cb` conta a partir da máscara).
fn abrir<'a, 'b>(l: &'b mut Leitor<'a>, mascara_de_8: bool) -> Option<(Bloco<'a, 'b>, usize)> {
    let _versao = l.u16()?;
    let cb = usize::from(l.u16()?);
    let fim = l.p + cb;
    let mascara = if mascara_de_8 { u64::from(l.u32()?) | (u64::from(l.u32()?) << 32) } else { u64::from(l.u32()?) };
    let base = l.p;
    Some((Bloco { l, base, mascara }, fim))
}

/// `GuidAndPicture`: o GUID e a `StdPicture` (preâmbulo, tamanho, bytes).
fn figura(l: &mut Leitor) -> Option<Vec<u8>> {
    l.bytes(16)?;
    let _preambulo = l.u32()?;
    let n = l.u32()? as usize;
    l.bytes(n).map(<[u8]>::to_vec)
}

/// `GuidAndFont`: pula (a letra do controle vem do `TextProps`).
fn pular_fonte(l: &mut Leitor) -> Option<()> {
    let guid = l.bytes(16)?;
    // StdFont {0BE35203-…}: versão, charset, flags, peso, altura, nome.
    if guid[..4] == [0x03, 0x52, 0xE3, 0x0B] {
        l.bytes(10)?;
        let n = usize::from(l.u8()?);
        l.bytes(n)?;
    } else {
        pular_textprops(l, &mut ControleVba::default())?;
    }
    Some(())
}

/// `TextProps`: a letra do controle.
fn pular_textprops(l: &mut Leitor, c: &mut ControleVba) -> Option<()> {
    let (mut b, fim) = abrir(l, false)?;
    let nome = b.tamanho(0);
    let efeitos = b.u32(1).unwrap_or(0);
    let altura = b.u32(2);
    let _ = b.u32(3);
    let _ = b.u8(4);
    let _ = b.u8(5);
    let alinhar = b.u8(6);
    let peso = b.u16(7).unwrap_or(0);
    b.fim_dos_dados();
    let _nome = b.texto(nome);
    c.letra = altura.map(|t| t as f32 / 20.0);
    c.negrito = efeitos & 1 != 0 || peso >= 700;
    c.italico = efeitos & 2 != 0;
    c.alinhar = match alinhar {
        Some(2) => "direita".into(),
        Some(3) => "centro".into(),
        _ => String::new(),
    };
    l.p = fim;
    Some(())
}

/// Um site (`OleSiteConcreteControl`): o que o formulário sabe de cada controle.
#[derive(Default)]
struct Site {
    nome: String,
    id: u32,
    bits: u32,
    tamanho_dos_dados: Option<u32>,
    ordem: u16,
    classe: u16,
    x: i32,
    y: i32,
    dica: String,
    fonte: String,
}

fn site(l: &mut Leitor) -> Option<Site> {
    let (mut b, fim) = abrir(l, false)?;
    let nome = b.tamanho(0);
    let tag = b.tamanho(1);
    let id = b.u32(2).unwrap_or(0);
    let _ajuda = b.u32(3);
    // SITE_FLAG padrão: TabStop, Visible, Streamed, AutoSize.
    let bits = b.u32(4).unwrap_or(0x33);
    let tamanho_dos_dados = b.u32(5);
    let ordem = b.u16(6).unwrap_or(0);
    let classe = b.u16(7).unwrap_or(0x7FFF);
    let _grupo = b.u16(9);
    let dica = b.tamanho(11);
    let licenca = b.tamanho(12);
    let origem = b.tamanho(13);
    let fonte = b.tamanho(14);
    b.fim_dos_dados();
    let nome = b.texto(nome);
    let _ = b.texto(tag);
    let (x, y) = b.par(8).unwrap_or((0, 0));
    let dica = b.texto(dica);
    let _ = b.texto(licenca);
    let _ = b.texto(origem);
    let fonte = b.texto(fonte);
    l.p = fim;
    Some(Site { nome, id, bits, tamanho_dos_dados, ordem, classe, x, y, dica, fonte })
}

/// O formulário (ou a moldura) do fluxo `f`: (título, fundo, tamanho em HIMETRIC, sites).
fn ler_f(f: &[u8]) -> Option<(String, String, (i32, i32), Vec<Site>)> {
    let mut l = Leitor::novo(f);
    let (mut b, fim) = abrir(&mut l, false)?;
    let fundo = b.u32(1).map(cor).unwrap_or_default();
    let _ = b.u32(2);
    let _ = b.u32(3);
    let booleanas = b.u32(6).unwrap_or(0);
    let _ = b.u8(7);
    let _ = b.u8(8);
    let _ = b.u8(9);
    let _ = b.u32(13);
    let icone = b.u16(15);
    let _ = b.u8(16);
    let _ = b.u8(17);
    let _ = b.u32(18);
    let titulo = b.tamanho(19);
    let fonte = b.u16(20);
    let quadro = b.u16(21);
    let _ = b.u32(22);
    let _ = b.u8(23);
    let _ = b.u8(25);
    let _ = b.u32(26);
    let _ = b.u32(27);
    b.fim_dos_dados();
    let tamanho = b.par(10).unwrap_or((0, 0));
    let _ = b.par(11);
    let _ = b.par(12);
    let titulo = b.texto(titulo);
    l.p = fim;
    if icone.is_some() {
        figura(&mut l)?;
    }
    if fonte.is_some() {
        pular_fonte(&mut l)?;
    }
    if quadro.is_some() {
        figura(&mut l)?;
    }
    // FORM_FLAG_DONTSAVECLASSTABLE: sem a tabela de classes.
    if booleanas & (1 << 15) == 0 {
        for _ in 0..l.u16()? {
            let _versao = l.u16()?;
            let cb = usize::from(l.u16()?);
            l.bytes(cb)?;
        }
    }
    let quantos = l.u32()? as usize;
    let bytes = l.u32()? as usize;
    let fim_dos_sites = l.p.checked_add(bytes)?;
    // Profundidade e tipo de cada site, alinhados a 4 no fim.
    let inicio = l.p;
    let mut faltam = quantos;
    while faltam > 0 {
        let _profundidade = l.u8()?;
        let misto = l.u8()?;
        let n = if misto & 0x80 != 0 {
            l.u8()?;
            usize::from(misto & 0x7F)
        } else {
            1
        };
        faltam = faltam.saturating_sub(n.max(1));
    }
    l.alinhar(inicio, 4);
    let mut sites = Vec::new();
    for _ in 0..quantos.min(MAX_CONTROLES) {
        if l.p >= fim_dos_sites {
            break;
        }
        sites.push(site(&mut l)?);
    }
    Some((titulo, fundo, tamanho, sites))
}

/// `MorphData`: caixa de texto, listas, caixa de seleção, opção e alternar.
fn morph(l: &mut Leitor, c: &mut ControleVba, classe: u16) -> Option<()> {
    let (mut b, fim) = abrir(l, true)?;
    let varios = b.u32(0).unwrap_or(0x2C80_481B);
    c.fundo = b.u32(1).map(cor).unwrap_or_default();
    c.cor = b.u32(2).map(cor).unwrap_or_default();
    c.max = b.u32(3).unwrap_or(0);
    let _ = b.u8(4);
    let _ = b.u8(5);
    let estilo = b.u8(6);
    let _ = b.u8(7);
    c.senha = b.u16(9).is_some_and(|p| p != 0);
    let _ = b.u32(10);
    let _ = b.u16(11);
    let _ = b.u16(12);
    let _ = b.u16(13);
    let _ = b.u16(14);
    let _ = b.u16(15);
    let _ = b.u8(16);
    let _ = b.u8(17);
    let _ = b.u8(18);
    let _ = b.u8(20);
    c.multipla = b.u8(21).is_some_and(|m| m != 0);
    let valor = b.tamanho(22);
    let titulo = b.tamanho(23);
    let _ = b.u32(24);
    let _ = b.u32(25);
    let _ = b.u32(26);
    let icone = b.u16(27);
    let quadro = b.u16(28);
    let _ = b.u16(29);
    let grupo = b.tamanho(32);
    b.fim_dos_dados();
    let tamanho = b.par(8);
    c.valor = b.texto(valor);
    c.texto = b.texto(titulo);
    c.grupo = b.texto(grupo);
    l.p = fim;
    if let Some((w, h)) = tamanho {
        (c.largura, c.altura) = (px(w), px(h));
    }
    c.desativado = varios & (1 << 1) == 0;
    c.multilinha = varios & (1 << 31) != 0;
    // O tipo vem da classe; a genérica (15) diz pelo DisplayStyle.
    c.tipo = match (classe, estilo.unwrap_or(1)) {
        (23, _) | (15, 1) => TipoVba::Texto,
        (24, _) | (15, 2) => TipoVba::Listagem,
        (25, _) | (15, 3 | 7) => TipoVba::Lista,
        (26, _) | (15, 4) => TipoVba::Caixa,
        (27, _) | (15, 5) => TipoVba::Opcao,
        (28, _) | (15, 6) => TipoVba::Alternar,
        (k, _) => TipoVba::Outro(k),
    };
    c.so_lista = estilo == Some(7);
    if icone.is_some() {
        figura(l)?;
    }
    if quadro.is_some() {
        figura(l)?;
    }
    pular_textprops(l, c)
}

fn botao(l: &mut Leitor, c: &mut ControleVba) -> Option<()> {
    let (mut b, fim) = abrir(l, false)?;
    c.cor = b.u32(0).map(cor).unwrap_or_default();
    c.fundo = b.u32(1).map(cor).unwrap_or_default();
    let varios = b.u32(2).unwrap_or(0x1B);
    let titulo = b.tamanho(3);
    let _ = b.u32(4);
    let _ = b.u8(6);
    let quadro = b.u16(7);
    let _ = b.u16(8);
    let icone = b.u16(10);
    b.fim_dos_dados();
    c.texto = b.texto(titulo);
    let tamanho = b.par(5);
    l.p = fim;
    if let Some((w, h)) = tamanho {
        (c.largura, c.altura) = (px(w), px(h));
    }
    c.desativado = varios & (1 << 1) == 0;
    c.tipo = TipoVba::Botao;
    if quadro.is_some() {
        figura(l)?;
    }
    if icone.is_some() {
        figura(l)?;
    }
    pular_textprops(l, c)
}

fn rotulo(l: &mut Leitor, c: &mut ControleVba) -> Option<()> {
    let (mut b, fim) = abrir(l, false)?;
    c.cor = b.u32(0).map(cor).unwrap_or_default();
    c.fundo = b.u32(1).map(cor).unwrap_or_default();
    let varios = b.u32(2).unwrap_or(0x80001B);
    let titulo = b.tamanho(3);
    let _ = b.u32(4);
    let _ = b.u8(6);
    let _ = b.u32(7);
    let _ = b.u16(8);
    let _ = b.u16(9);
    let quadro = b.u16(10);
    let _ = b.u16(11);
    let icone = b.u16(12);
    b.fim_dos_dados();
    c.texto = b.texto(titulo);
    let tamanho = b.par(5);
    l.p = fim;
    if let Some((w, h)) = tamanho {
        (c.largura, c.altura) = (px(w), px(h));
    }
    c.desativado = varios & (1 << 1) == 0;
    c.tipo = TipoVba::Rotulo;
    if quadro.is_some() {
        figura(l)?;
    }
    if icone.is_some() {
        figura(l)?;
    }
    pular_textprops(l, c)
}

fn imagem(l: &mut Leitor, c: &mut ControleVba) -> Option<()> {
    let (mut b, fim) = abrir(l, false)?;
    let _ = b.u32(3);
    c.fundo = b.u32(4).map(cor).unwrap_or_default();
    let _ = b.u8(5);
    let _ = b.u8(6);
    let _ = b.u8(7);
    let _ = b.u8(8);
    let quadro = b.u16(10);
    let _ = b.u8(11);
    let _ = b.u32(13);
    let icone = b.u16(14);
    b.fim_dos_dados();
    let tamanho = b.par(9);
    l.p = fim;
    if let Some((w, h)) = tamanho {
        (c.largura, c.altura) = (px(w), px(h));
    }
    c.tipo = TipoVba::Imagem;
    if quadro.is_some() {
        c.figura = figura(l)?;
    }
    if icone.is_some() {
        figura(l)?;
    }
    Some(())
}

/// Lê um formulário a partir dos fluxos. `subpasta(nome)` devolve o `f` e o `o` de uma
/// subpasta (`i05`), onde moram os controles de cada moldura.
pub fn ler(nome: &str, vbframe: &str, f: &[u8], o: &[u8], subpasta: &dyn Fn(&str) -> Option<(Vec<u8>, Vec<u8>)>) -> Option<FormularioVba> {
    let (_, fundo, (w, h), _) = ler_f(f)?;
    let mut form = FormularioVba { nome: nome.to_string(), fundo, largura: px(w), altura: px(h), ..Default::default() };
    form.titulo = vbframe
        .lines()
        .find_map(|l| l.trim().strip_prefix("Caption").and_then(|r| r.trim_start().strip_prefix('=')))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| nome.to_string());
    controles(f, o, (0.0, 0.0), subpasta, 0, &mut form.controles);
    Some(form)
}

/// Os formulários de um `vbaProject.bin`: a pasta de cada módulo de classe que tem `f` e `o`.
pub fn ler_do_projeto(bin: &[u8], modulos: &[crate::ovba::ModuloVba]) -> Vec<FormularioVba> {
    use std::io::Read;
    let Ok(mut cfb) = cfb::CompoundFile::open(std::io::Cursor::new(bin)) else { return Vec::new() };
    let mut fluxo = |caminho: &str| -> Option<Vec<u8>> {
        let mut v = Vec::new();
        cfb.open_stream(caminho).ok()?.take(crate::ovba::LIMITE_DE_BYTES as u64).read_to_end(&mut v).ok()?;
        Some(v)
    };
    let mut saida = Vec::new();
    for m in modulos.iter().filter(|m| m.tipo == crate::ovba::TipoDeModulo::Classe) {
        let raiz = format!("/{}", m.nome);
        let (Some(f), Some(o)) = (fluxo(&format!("{raiz}/f")), fluxo(&format!("{raiz}/o"))) else { continue };
        let vbframe = fluxo(&format!("{raiz}/\u{3}VBFrame")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        // As subpastas das molduras, lidas antes (o leitor do cfb não entra duas vezes).
        let mut subpastas = std::collections::HashMap::new();
        for id in 0..256u32 {
            for nome in [format!("i{id:02}"), format!("i{id}")] {
                if let (Some(sf), Some(so)) = (fluxo(&format!("{raiz}/{nome}/f")), fluxo(&format!("{raiz}/{nome}/o"))) {
                    subpastas.insert(nome, (sf, so));
                }
            }
        }
        let sub = |nome: &str| subpastas.get(nome).cloned();
        if let Some(form) = ler(&m.nome, &vbframe, &f, &o, &sub) {
            saida.push(form);
        }
    }
    saida
}

fn controles(f: &[u8], o: &[u8], origem: (f32, f32), subpasta: &dyn Fn(&str) -> Option<(Vec<u8>, Vec<u8>)>, nivel: usize, saida: &mut Vec<ControleVba>) {
    let Some((_, _, _, sites)) = ler_f(f) else { return };
    let mut pos = 0usize;
    for s in sites {
        if saida.len() >= MAX_CONTROLES {
            return;
        }
        let mut c = ControleVba {
            nome: s.nome.clone(),
            x: origem.0 + px(s.x),
            y: origem.1 + px(s.y),
            oculto: s.bits & (1 << 1) == 0,
            padrao: s.bits & (1 << 2) != 0,
            cancelar: s.bits & (1 << 3) != 0,
            dica: s.dica.clone(),
            fonte: s.fonte.clone(),
            ordem: s.ordem,
            ..Default::default()
        };
        // Os dados no `o`, pelo tamanho que o site diz (um controle ilegível não leva os outros).
        let dados = s.tamanho_dos_dados.and_then(|n| {
            let fatia = o.get(pos..pos.checked_add(n as usize)?);
            pos += n as usize;
            fatia
        });
        let lido = dados.and_then(|d| {
            let mut l = Leitor::novo(d);
            match s.classe {
                15 | 23..=28 => morph(&mut l, &mut c, s.classe),
                17 => botao(&mut l, &mut c),
                21 => rotulo(&mut l, &mut c),
                12 => imagem(&mut l, &mut c),
                _ => None,
            }
        });
        if s.classe == 14 {
            c.tipo = TipoVba::Moldura;
            // A moldura: o próprio formulário na subpasta `i<ID>`, com título e tamanho.
            let pasta = subpasta(&format!("i{:02}", s.id)).or_else(|| subpasta(&format!("i{}", s.id)));
            if let Some((sf, so)) = pasta {
                if let Some((titulo, fundo, (w, h), _)) = ler_f(&sf) {
                    (c.texto, c.fundo, c.largura, c.altura) = (titulo, fundo, px(w), px(h));
                }
                saida.push(c.clone());
                if nivel < MAX_PROFUNDIDADE {
                    // Os de dentro ficam abaixo da faixa do título da moldura.
                    controles(&sf, &so, (c.x, c.y + 8.0), subpasta, nivel + 1, saida);
                }
                continue;
            }
        } else if lido.is_none() {
            c.tipo = TipoVba::Outro(s.classe);
        }
        saida.push(c);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    /// O `UserForm1` do `CheckOptionToggleValue.xls` (arquivos de teste do LibreOffice).
    const F1: &str = "00 04 20 00 08 0c 00 0c 03 00 00 00 03 00 00 00 00 7d 00 00 74 20 00 00 7f 15 00 00 00 00 00 00
        00 00 00 00 00 00 03 00 00 00 90 00 00 00 00 83 01 04 00 00 2c 00 e5 01 00 00 0d 00 00 80 01 00
        00 00 58 00 00 00 00 00 1b 00 4f 70 74 69 6f 6e 42 75 74 74 6f 6e 31 00 00 00 4f 03 00 00 4f 03
        00 00 00 00 2c 00 e5 01 00 00 0d 00 00 80 02 00 00 00 58 00 00 00 01 00 1c 00 54 6f 67 67 6c 65
        42 75 74 74 6f 6e 31 00 00 00 7b 02 00 00 45 08 00 00 00 00 28 00 e5 01 00 00 09 00 00 80 03 00
        00 00 54 00 00 00 02 00 1a 00 43 68 65 63 6b 42 6f 78 31 74 6f 6e 4f 03 00 00 e2 0e 00 00";
    const O1: &str = "00 02 38 00 46 01 c0 80 00 00 00 00 0f 00 00 80 12 00 00 80 05 00 00 00 01 00 00 80 0d 00 00 80
        bf 0a 00 00 7b 02 00 00 30 35 1f 04 4f 70 74 69 6f 6e 42 75 74 74 6f 6e 31 00 00 00 00 02 18 00
        35 00 00 00 06 00 00 80 a5 00 00 00 00 02 00 00 54 61 68 6f 6d 61 00 00 00 02 38 00 46 01 c0 80
        00 00 00 00 0f 00 00 80 12 00 00 80 06 00 00 00 01 00 00 80 0d 00 00 80 67 0c 00 00 4e 03 00 00
        30 35 1f 04 54 6f 67 67 6c 65 42 75 74 74 6f 6e 31 00 00 00 00 02 18 00 75 00 00 00 06 00 00 80
        a5 00 00 00 00 02 03 00 54 61 68 6f 6d 61 00 00 00 02 34 00 46 01 c0 80 00 00 00 00 0f 00 00 80
        12 00 00 80 04 00 00 00 01 00 00 80 09 00 00 80 67 0c 00 00 4f 03 00 00 30 35 1f 04 43 68 65 63
        6b 42 6f 78 31 74 6f 6e 00 02 18 00 35 00 00 00 06 00 00 80 a5 00 00 00 00 02 00 00 54 61 68 6f
        6d 61 00 00";
    const VBFRAME: &str = "VERSION 5.00\r\nBegin {C62A69F0-16DC-11CE-9E98-00AA00574A4F} UserForm1 \r\n   Caption         =   \"Cadastro\"\r\n   ClientHeight    =   3120\r\nEnd\r\n";

    /// O `UserForm1` do `MiscControlTests.xls`: caixa de texto, ComboBox com RowSource e ListBox.
    const F2: &str = "00 04 20 00 08 0c 00 0c 05 00 00 00 07 00 00 00 00 7d 00 00 74 20 00 00 7f 15 00 00 00 00 00 00
        00 00 00 00 00 00 03 00 00 00 8c 00 00 00 00 83 01 00 00 00 24 00 e5 01 00 00 08 00 00 80 01 00
        00 00 34 00 00 00 00 00 17 00 54 65 78 74 42 6f 78 31 d4 00 00 00 f6 04 00 00 00 00 34 00 e5 41
        00 00 09 00 00 80 05 00 00 00 38 00 00 00 02 00 19 00 07 00 00 80 43 6f 6d 62 6f 42 6f 78 31 00
        00 00 31 12 00 00 22 04 00 00 66 32 35 3a 66 32 38 78 00 00 24 00 e5 01 00 00 08 00 00 80 04 00
        00 00 34 00 00 00 01 00 18 00 4c 69 73 74 42 6f 78 31 d4 00 00 00 93 0b 00 00";
    const O2: &str = "00 02 14 00 01 01 00 80 00 00 00 00 1b 48 80 2c 3a 0d 00 00 22 04 00 00 00 02 18 00 35 00 00 00
        06 00 00 80 a5 00 00 00 00 02 00 00 54 61 68 6f 6d 61 20 00 00 02 18 00 41 01 05 80 00 00 00 00
        1b 48 80 2c 03 01 02 00 93 0b 00 00 84 03 00 00 00 02 18 00 35 00 00 00 06 00 00 80 a5 00 00 00
        00 02 00 00 54 61 68 6f 6d 61 20 00 00 02 14 00 60 01 01 80 00 00 00 00 03 02 00 00 0e 0e 00 00
        9e 06 00 00 00 02 18 00 35 00 00 00 06 00 00 80 a5 00 00 00 00 02 00 00 54 61 68 6f 6d 61 20 00";

    #[test]
    fn opcao_alternar_e_caixa_com_texto_valor_e_posicao() {
        let f = ler("UserForm1", VBFRAME, &hex(F1), &hex(O1), &|_| None).unwrap();
        assert_eq!(f.titulo, "Cadastro");
        assert_eq!((f.largura, f.altura), (314.0, 208.0));
        let nomes: Vec<_> = f.controles.iter().map(|c| (c.nome.as_str(), c.tipo, c.texto.as_str(), c.valor.as_str())).collect();
        assert_eq!(nomes, vec![
            ("OptionButton1", TipoVba::Opcao, "OptionButton1", "0"),
            ("ToggleButton1", TipoVba::Alternar, "ToggleButton1", "0"),
            ("CheckBox1", TipoVba::Caixa, "CheckBox1", "0"),
        ]);
        let o = &f.controles[0];
        assert_eq!((o.x, o.y, o.largura, o.altura), (32.0, 32.0, 104.0, 24.0));
        assert_eq!(o.letra, Some(8.25));
        assert!(!o.desativado && !o.oculto);
        assert_eq!(f.controles[2].y, 144.0);
    }

    #[test]
    fn texto_lista_com_rowsource_e_listagem() {
        let f = ler("UserForm1", "", &hex(F2), &hex(O2), &|_| None).unwrap();
        assert_eq!(f.titulo, "UserForm1");
        let c: Vec<_> = f.controles.iter().map(|c| (c.nome.as_str(), c.tipo, c.fonte.as_str())).collect();
        assert_eq!(c, vec![
            ("TextBox1", TipoVba::Texto, ""),
            ("ComboBox1", TipoVba::Lista, "f25:f28"),
            ("ListBox1", TipoVba::Listagem, ""),
        ]);
        let t = &f.controles[0];
        assert_eq!((t.largura, t.altura), (128.0, 40.0));
        assert!(!t.multilinha && !t.desativado);
    }

    #[test]
    fn bytes_ruins_nao_derrubam() {
        assert!(ler("X", "", &[0, 4, 0xff, 0xff], &[], &|_| None).is_none());
        let mut f = hex(F1);
        f.truncate(120);
        let _ = ler("X", "", &f, &hex(O1), &|_| None);
        let _ = ler("X", "", &hex(F1), &hex(O1)[..30], &|_| None);
    }
}
