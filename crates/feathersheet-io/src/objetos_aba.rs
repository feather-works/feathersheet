//! Objetos soltos sobre a grade: formas, ícones, WordArt e SmartArt.
//!
//! Moram no campo opaco `ob` da aba ([`super::sheet::Sheet::ob`]), em JSON:
//!
//! ```json
//! {"v":1,"objetos":[
//!   {"id":1,"tipo":"forma","forma":"retangulo",
//!    "de":{"linha":2,"coluna":1,"dx":12.0,"dy":4.0},
//!    "ate":{"linha":6,"coluna":3,"dx":30.0,"dy":10.0},
//!    "giro":0.0,
//!    "aparencia":{"preenchimento":"#4472c4","contorno":"#2f528f","espessura":1.0,"transparencia":0.0},
//!    "texto":"Etapa 1"}
//! ]}
//! ```
//!
//! - `tipo`: `"forma"`, `"icone"`, `"wordart"`, `"smartart"` ou `"imagem"`
//!   (figura na ordem das formas, bytes em `imagem.src`, como as do `.xlsx`).
//! - Do Office (só quando há): `gradiente` (`{"angulo":90.0,"paradas":[[0.0,"#88c43a"],[1.0,"#385723"]]}`),
//!   `efeito_sombra` (`{"cor":"#00000066","desfoque":6.7,"dx":0.0,"dy":0.0}`),
//!   `fonte` (`{"tamanho":20.0,"negrito":true,"cor":"#ffffff","familia":"Calibri"}`).
//! - `forma`: o id da forma no catálogo do `feather-shapes`
//!   (`TipoForma::id`: `retangulo`, `retangulo-arredondado`, `elipse`,
//!   `seta-direita`, `estrela-5`, `linha`…), o mesmo que o FeatherSlide
//!   grava; o nome `prstGeom` do DrawingML (`rect`, `rightArrow`…) também é
//!   aceito na leitura. Ícone: o id do ícone Tabler; SmartArt usará o id do layout.
//! - WordArt (`"tipo":"wordart"`): o texto fica em `texto` (editável, F2);
//!   `forma` é a deformação (`feather_shapes::wordart::Transformacao::nome`:
//!   `reto`, `arco`, `onda`, `inclinado`) e `ajustes[0]` a intensidade, de
//!   -1 a 1. Preenchimento, contorno e espessura das letras vêm de
//!   `aparencia`; duas chaves próprias (só no WordArt) guardam o resto do
//!   estilo pronto: `"estilo"` (a chave do estilo da galeria comum,
//!   `feather_shapes::wordart::ESTILOS`: `solido`, `contornado`, `dourado`,
//!   `rosa`) e `"sombra"` (`{"cor":"#rrggbbaa","dx":3.0,"dy":3.0}`, em px
//!   lógicos; ausente = sem sombra). Exemplo:
//!   `{"id":9,"tipo":"wordart","forma":"arco","ajustes":[0.5],"texto":"Vendas",
//!   "aparencia":{"preenchimento":"#fbbc04","contorno":"#b06000","espessura":1.0},
//!   "estilo":"dourado","sombra":{"cor":"#20212480","dx":3.0,"dy":3.0},…}`.
//! - `de`/`ate`: âncora de **duas células** (canto superior esquerdo e
//!   inferior direito, como o `twoCellAnchor` do Excel em "mover e
//!   redimensionar com as células"). `linha`/`coluna` em base 0; `dx`/`dy`
//!   em pixels lógicos (zoom 100 %) a partir do início da célula. Mudar a
//!   largura de uma coluna no meio da forma estica a forma; mudar uma antes
//!   dela a desloca.
//! - `giro`: graus no sentido horário, em torno do centro.
//! - `espelhar_h`/`espelhar_v` (só quando `true`): forma espelhada.
//! - `ajustes` (só quando há): os números das alças de ajuste, no sentido
//!   de cada forma — `rect`/`roundRect`: raio dos cantos como fração do
//!   lado menor (um valor para todos, ou quatro: sup. esq., sup. dir.,
//!   inf. dir., inf. esq.); `rightArrow`: [altura da haste, comprimento da
//!   ponta]; `star5`: [profundidade das pontas]; `triangle`: [posição do
//!   vértice].
//! - `alinhar` (só quando não é o padrão): onde o texto fica na forma,
//!   `{"h":"esquerda|centro|direita","v":"topo|meio|baixo"}`.
//! - `macro` (só quando há): nome da macro que o clique na forma executa.
//! - `aparencia`: cores `#rrggbb` (ausente = sem preenchimento / sem
//!   contorno), espessura do contorno em px, transparência de 0 (opaco) a 1.
//! - Chaves desconhecidas são preservadas (o `feather-shapes` vai
//!   acrescentar ajustes, efeitos e estilo de texto sem quebrar o arquivo).
//!
//! A ordem da lista é a ordem de empilhamento (o último fica por cima).
//! O motor (`feathersheet-core`) só guarda o texto e registra cada operação
//! como UM Ctrl+Z (`Sheet::alterar_objetos`); o modelo, a validação e a
//! conversão âncora ↔ retângulo ficam aqui, como a tinta (`tinta_aba`).
use feathersheet_core::planilha::sheet::Sheet;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Versão do JSON do campo `ob`.
pub const VERSAO: u32 = 1;
/// Teto de objetos por aba (o arquivo não cresce sem limite).
pub const MAX_OBJETOS: usize = 2000;
/// Lado mínimo de um objeto ao redimensionar, em px lógicos.
pub const LADO_MINIMO: f32 = 4.0;

/// O que o objeto é. A geometria de cada um vem do catálogo comum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoObjeto {
    #[default]
    Forma,
    Icone,
    /// Texto artístico: `texto` deformado por `forma` (ver o cabeçalho).
    Wordart,
    Smartart,
    /// Figura (PNG/JPEG) dentro da camada de objetos, na ordem de
    /// empilhamento das formas (a do `.xlsx` vem assim). Os bytes ficam em
    /// [`ObjetoSolto::imagem`].
    Imagem,
    /// Controle de formulário do Excel (botão, botão de girar, caixa de seleção…): ver
    /// [`ControleFormulario`]. O rótulo fica em `texto`, a macro em `macro`.
    Controle,
}

/// O tipo do controle de formulário (`ObjectType` do VML do Excel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoControleForm {
    Botao,
    Girar,
    Caixa,
    Opcao,
    Combo,
    Lista,
    Rolagem,
    Rotulo,
    Grupo,
}

impl TipoControleForm {
    /// `ObjectType` do VML.
    pub fn do_excel(t: &str) -> Option<Self> {
        Some(match t {
            "Button" => Self::Botao,
            "Spin" => Self::Girar,
            "Checkbox" => Self::Caixa,
            "Radio" => Self::Opcao,
            "Drop" => Self::Combo,
            "List" => Self::Lista,
            "Scroll" => Self::Rolagem,
            "Label" => Self::Rotulo,
            "GBox" => Self::Grupo,
            _ => return None,
        })
    }

    pub fn para_excel(self) -> &'static str {
        match self {
            Self::Botao => "Button",
            Self::Girar => "Spin",
            Self::Caixa => "Checkbox",
            Self::Opcao => "Radio",
            Self::Combo => "Drop",
            Self::Lista => "List",
            Self::Rolagem => "Scroll",
            Self::Rotulo => "Label",
            Self::Grupo => "GBox",
        }
    }
}

/// Um controle de formulário do Excel, com os mesmos campos do VML (`x:ClientData`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControleFormulario {
    pub tipo: TipoControleForm,
    /// Célula ligada (`FmlaLink`, como no arquivo: `$C$8` ou `Aba!$C$8`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub celula: Option<String>,
    /// Intervalo de entrada da lista/caixa de combinação (`FmlaRange`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faixa: Option<String>,
    /// Valor atual (`Val`; na lista e na combinação, o item escolhido, `Sel`, a partir de 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    /// Quanto cada seta soma (`Inc`) e cada clique no trilho (`Page`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passo: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pagina: Option<i64>,
    /// Caixa e opção: 0 desmarcada, 1 marcada, 2 misto (`Checked`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marcado: Option<u8>,
    /// Linhas da lista aberta da caixa de combinação (`DropLines`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linhas: Option<u32>,
    /// Barra de rolagem deitada (`Horiz`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub horizontal: bool,
    /// A macro como o Excel grava (`FmlaMacro`: `[0]!AbrirPainel`), para o `.xlsm` voltar igual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_excel: Option<String>,
}

impl ControleFormulario {
    pub fn novo(tipo: TipoControleForm) -> Self {
        Self { tipo, celula: None, faixa: None, valor: None, min: None, max: None, passo: None, pagina: None,
            marcado: None, linhas: None, horizontal: false, macro_excel: None }
    }

    /// Botão de girar e barra de rolagem: o valor depois de andar `passos` setas (+/-) ou páginas,
    /// preso entre o mínimo e o máximo, como no Excel (0 a 100 e passo 1 quando o arquivo não diz).
    pub fn andar(&self, passos: i64, pagina: bool) -> i64 {
        let (min, max) = (self.min.unwrap_or(0), self.max.unwrap_or(100));
        let passo = if pagina { self.pagina.unwrap_or(10) } else { self.passo.unwrap_or(1) };
        (self.valor.unwrap_or(min) + passos * passo).clamp(min.min(max), max.max(min))
    }
}

/// Degradê do preenchimento (o `gradFill` do Office).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gradiente {
    /// `false` = linear; `true` = radial (do centro para fora).
    #[serde(default, skip_serializing_if = "nao")]
    pub radial: bool,
    /// Direção do linear em graus, no sentido do Office: 0 = da esquerda
    /// para a direita, 90 = de cima para baixo.
    #[serde(default)]
    pub angulo: f32,
    /// Paradas `(posição 0..1, #rrggbb[aa])`, em ordem.
    pub paradas: Vec<(f32, String)>,
}

/// Sombra externa (`outerShdw`), em px lógicos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sombra {
    /// `#rrggbbaa`.
    pub cor: String,
    #[serde(default)]
    pub desfoque: f32,
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
}

/// Letra do texto da forma (ausente = 11 pt, cor automática).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FonteTexto {
    /// Tamanho em pontos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tamanho: Option<f32>,
    #[serde(default, skip_serializing_if = "nao")]
    pub negrito: bool,
    #[serde(default, skip_serializing_if = "nao")]
    pub italico: bool,
    /// `#rrggbb[aa]`; ausente = branco sobre forma cheia, cinza-escuro sem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cor: Option<String>,
    /// Família (`Calibri`); ausente = a da interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub familia: Option<String>,
    /// Não quebrar o texto na largura da forma (`wrap="none"`).
    #[serde(default, skip_serializing_if = "nao")]
    pub sem_quebra: bool,
}

/// Bytes de uma figura e o ajuste de cor dela.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagemObjeto {
    /// `data:image/png;base64,…` (ou jpeg).
    pub src: String,
    /// Duotom do Office (`a:duotone`): `[escura, clara]`; a luminância de
    /// cada pixel escolhe entre as duas, o alfa fica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duotom: Option<[String; 2]>,
    /// Recorte (`srcRect`) em frações `[esquerda, topo, direita, base]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorte: Option<[f32; 4]>,
}

/// Ponto preso a uma célula: linha/coluna (base 0) e deslocamento em px
/// lógicos dentro dela.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Ancora {
    pub linha: u32,
    pub coluna: u32,
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
}

/// Retângulo em pixels lógicos absolutos da aba (zoom 100 %, desde o canto
/// de A1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Retangulo {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}

impl Retangulo {
    pub const fn novo(x: f32, y: f32, largura: f32, altura: f32) -> Self {
        Self { x, y, largura, altura }
    }
    /// O retângulo entre dois cantos quaisquer (largura/altura positivas).
    pub fn entre(ax: f32, ay: f32, bx: f32, by: f32) -> Self {
        Self::novo(ax.min(bx), ay.min(by), (bx - ax).abs(), (by - ay).abs())
    }
    pub fn centro(self) -> (f32, f32) {
        (self.x + self.largura / 2.0, self.y + self.altura / 2.0)
    }
    pub fn valido(self) -> bool {
        [self.x, self.y, self.largura, self.altura].iter().all(|v| v.is_finite())
            && self.largura >= 0.0
            && self.altura >= 0.0
    }
}

/// A régua da aba: onde começa cada coluna/linha e qual célula fica num
/// ponto. Quem desenha (a GUI) conhece larguras padrão, ocultas etc.
pub trait Regua {
    /// Início da coluna `c` em px lógicos desde A1.
    fn inicio_coluna(&self, c: u32) -> f32;
    /// Início da linha `r` em px lógicos desde A1.
    fn inicio_linha(&self, r: u32) -> f32;
    /// Coluna que contém `x` (a última com início <= x).
    fn coluna_em(&self, x: f32) -> u32;
    /// Linha que contém `y`.
    fn linha_em(&self, y: f32) -> u32;
}

impl Ancora {
    /// O ponto absoluto da âncora. O deslocamento não passa do fim da
    /// célula: se a coluna encolheu, o canto encosta na borda dela (Excel).
    pub fn ponto(&self, regua: &impl Regua) -> (f32, f32) {
        let x0 = regua.inicio_coluna(self.coluna);
        let y0 = regua.inicio_linha(self.linha);
        let largura = (regua.inicio_coluna(self.coluna.saturating_add(1)) - x0).max(0.0);
        let altura = (regua.inicio_linha(self.linha.saturating_add(1)) - y0).max(0.0);
        (x0 + self.dx.clamp(0.0, largura), y0 + self.dy.clamp(0.0, altura))
    }

    /// A âncora da célula onde fica o ponto absoluto.
    pub fn em(x: f32, y: f32, regua: &impl Regua) -> Self {
        let (x, y) = (x.max(0.0), y.max(0.0));
        let coluna = regua.coluna_em(x);
        let linha = regua.linha_em(y);
        Self {
            linha,
            coluna,
            dx: arredondar(x - regua.inicio_coluna(coluna)),
            dy: arredondar(y - regua.inicio_linha(linha)),
        }
    }
}

/// Duas casas decimais bastam (px lógicos) e deixam o JSON curto.
fn arredondar(v: f32) -> f32 {
    if v.is_finite() { (v * 100.0).round() / 100.0 } else { 0.0 }
}

/// Preenchimento, contorno e transparência.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aparencia {
    /// `#rrggbb`; `None` = sem preenchimento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preenchimento: Option<String>,
    /// `#rrggbb`; `None` = sem contorno.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contorno: Option<String>,
    /// Espessura do contorno em px lógicos.
    #[serde(default = "espessura_padrao")]
    pub espessura: f32,
    /// 0 = opaco, 1 = invisível. Vale para preenchimento e contorno.
    #[serde(default)]
    pub transparencia: f32,
}

fn espessura_padrao() -> f32 {
    1.0
}

impl Default for Aparencia {
    /// A forma nova do Office: azul com contorno azul-escuro.
    fn default() -> Self {
        Self {
            preenchimento: Some("#4472c4".into()),
            contorno: Some("#2f528f".into()),
            espessura: 1.0,
            transparencia: 0.0,
        }
    }
}

impl Aparencia {
    /// Aparência padrão para a forma do catálogo (linha não tem preenchimento).
    pub fn padrao_para(forma: &str) -> Self {
        if e_linha(forma) {
            Self { preenchimento: None, contorno: Some("#4472c4".into()), espessura: 2.0, transparencia: 0.0 }
        } else {
            Self::default()
        }
    }
}

/// A forma é uma linha aberta (sem preenchimento, clique pela linha).
pub fn e_linha(forma: &str) -> bool {
    matches!(
        forma,
        "linha" | "conector-reto" | "conector-cotovelo" | "conector-curvo" | "arco"
            | "line" | "straightConnector1" | "bentConnector3" | "curvedConnector3" | "arc"
    )
}

/// `#rrggbb` (ou `#rrggbbaa`) válido.
pub fn cor_valida(hex: &str) -> bool {
    let s = hex.trim().trim_start_matches('#');
    (s.len() == 6 || s.len() == 8) && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn nao(v: &bool) -> bool {
    !*v
}

/// Alinhamento horizontal do texto dentro da forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlinharH {
    Esquerda,
    #[default]
    Centro,
    Direita,
}

/// Alinhamento vertical do texto dentro da forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlinharV {
    Topo,
    #[default]
    Meio,
    Baixo,
}

/// Onde o texto fica dentro da forma (padrão: centro e meio, como o Office).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct AlinharTexto {
    #[serde(default)]
    pub h: AlinharH,
    #[serde(default)]
    pub v: AlinharV,
}

impl AlinharTexto {
    pub fn e_padrao(&self) -> bool {
        *self == Self::default()
    }
}

/// Um objeto solto sobre a grade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjetoSolto {
    pub id: u64,
    #[serde(default)]
    pub tipo: TipoObjeto,
    /// Id no catálogo (`prstGeom` para formas).
    pub forma: String,
    pub de: Ancora,
    pub ate: Ancora,
    /// Graus, sentido horário.
    #[serde(default)]
    pub giro: f32,
    /// Espelhada na horizontal / na vertical (a linha desenhada de baixo
    /// para cima é uma linha espelhada na vertical, como o `flipV` do Office).
    #[serde(default, skip_serializing_if = "nao")]
    pub espelhar_h: bool,
    #[serde(default, skip_serializing_if = "nao")]
    pub espelhar_v: bool,
    /// Ajustes da forma (as alças de ajuste do Office, `avLst`): uma lista
    /// de números cujo sentido depende da forma. Vazia = os padrões.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ajustes: Vec<f32>,
    #[serde(default)]
    pub aparencia: Aparencia,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texto: Option<String>,
    /// Alinhamento do texto na forma (só gravado quando não é centro/meio).
    #[serde(default, skip_serializing_if = "AlinharTexto::e_padrao")]
    pub alinhar: AlinharTexto,
    /// Macro atribuída: o clique na forma a executa (Ctrl+clique seleciona).
    #[serde(rename = "macro", default, skip_serializing_if = "Option::is_none")]
    pub macro_nome: Option<String>,
    /// Degradê no lugar do preenchimento sólido (o sólido fica de reserva).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradiente: Option<Gradiente>,
    /// Sombra externa (chave própria: `sombra` é a do WordArt, em `extras`).
    #[serde(rename = "efeito_sombra", default, skip_serializing_if = "Option::is_none")]
    pub sombra: Option<Sombra>,
    /// Letra do texto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fonte: Option<FonteTexto>,
    /// Só no `tipo: "imagem"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imagem: Option<ImagemObjeto>,
    /// Só no `tipo: "controle"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controle: Option<ControleFormulario>,
    /// Chaves que esta versão não conhece: atravessam intactas.
    #[serde(flatten)]
    pub extras: Map<String, Value>,
}

impl ObjetoSolto {
    /// Forma nova no retângulo absoluto `ret`, com a aparência padrão.
    pub fn nova_forma(id: u64, forma: &str, ret: Retangulo, regua: &impl Regua) -> Self {
        let mut o = Self {
            id,
            tipo: TipoObjeto::Forma,
            forma: forma.to_owned(),
            de: Ancora::default(),
            ate: Ancora::default(),
            giro: 0.0,
            espelhar_h: false,
            espelhar_v: false,
            ajustes: Vec::new(),
            aparencia: Aparencia::padrao_para(forma),
            texto: None,
            alinhar: AlinharTexto::default(),
            macro_nome: None,
            gradiente: None,
            sombra: None,
            fonte: None,
            imagem: None,
            controle: None,
            extras: Map::new(),
        };
        o.ancorar(ret, regua);
        o
    }

    /// Controle de formulário preso às células `de`/`ate`.
    pub fn novo_controle(controle: ControleFormulario, de: Ancora, ate: Ancora) -> Self {
        Self {
            id: 0, tipo: TipoObjeto::Controle, forma: "retangulo".into(), de, ate, giro: 0.0,
            espelhar_h: false, espelhar_v: false, ajustes: Vec::new(), aparencia: Aparencia::default(),
            texto: None, alinhar: AlinharTexto::default(), macro_nome: None, gradiente: None, sombra: None,
            fonte: None, imagem: None, controle: Some(controle), extras: Map::new(),
        }
    }

    /// O retângulo absoluto (sem o giro) pela régua atual.
    pub fn retangulo(&self, regua: &impl Regua) -> Retangulo {
        let (ax, ay) = self.de.ponto(regua);
        let (bx, by) = self.ate.ponto(regua);
        Retangulo::entre(ax, ay, bx, by)
    }

    /// Prende o objeto às células do retângulo absoluto `ret`.
    pub fn ancorar(&mut self, ret: Retangulo, regua: &impl Regua) {
        if !ret.valido() {
            return;
        }
        self.de = Ancora::em(ret.x, ret.y, regua);
        self.ate = Ancora::em(ret.x + ret.largura, ret.y + ret.altura, regua);
    }

    /// Giro normalizado em `[0, 360)`.
    pub fn definir_giro(&mut self, graus: f32) {
        if graus.is_finite() {
            self.giro = arredondar(graus.rem_euclid(360.0));
        }
    }
}

/// Todos os objetos de uma aba.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjetosAba {
    pub v: u32,
    #[serde(default)]
    pub objetos: Vec<ObjetoSolto>,
}

impl Default for ObjetosAba {
    fn default() -> Self {
        Self { v: VERSAO, objetos: Vec::new() }
    }
}

impl ObjetosAba {
    /// Lê o campo `ob`. Vazio/ausente = sem objetos.
    pub fn ler(ob: Option<&str>) -> Result<Self, String> {
        let Some(texto) = ob.filter(|t| !t.trim().is_empty()) else { return Ok(Self::default()) };
        let lido: Self = serde_json::from_str(texto).map_err(|e| format!("Objetos da aba inválidos: {e}"))?;
        if lido.v != VERSAO {
            return Err(format!("Versão {} dos objetos da aba desconhecida.", lido.v));
        }
        if lido.objetos.len() > MAX_OBJETOS {
            return Err(format!("Mais de {MAX_OBJETOS} objetos numa aba."));
        }
        Ok(lido)
    }

    /// Texto do campo `ob`; `None` se não houver objeto.
    pub fn gravar(&self) -> Option<String> {
        if self.objetos.is_empty() {
            return None;
        }
        serde_json::to_string(self).ok()
    }

    pub fn proximo_id(&self) -> u64 {
        self.objetos.iter().map(|o| o.id).max().unwrap_or(0) + 1
    }

    pub fn obter(&self, id: u64) -> Option<&ObjetoSolto> {
        self.objetos.iter().find(|o| o.id == id)
    }

    pub fn obter_mut(&mut self, id: u64) -> Option<&mut ObjetoSolto> {
        self.objetos.iter_mut().find(|o| o.id == id)
    }

    /// Acrescenta por cima dos outros; devolve o id (`None` no teto).
    pub fn adicionar(&mut self, mut objeto: ObjetoSolto) -> Option<u64> {
        if self.objetos.len() >= MAX_OBJETOS {
            return None;
        }
        if objeto.id == 0 || self.obter(objeto.id).is_some() {
            objeto.id = self.proximo_id();
        }
        let id = objeto.id;
        self.objetos.push(objeto);
        Some(id)
    }

    pub fn remover(&mut self, id: u64) -> Option<ObjetoSolto> {
        let i = self.objetos.iter().position(|o| o.id == id)?;
        Some(self.objetos.remove(i))
    }

    /// Leva o objeto para cima de todos (`true`) ou para baixo (`false`).
    /// Devolve se a ordem mudou.
    pub fn reordenar(&mut self, id: u64, para_frente: bool) -> bool {
        let Some(i) = self.objetos.iter().position(|o| o.id == id) else { return false };
        let destino = if para_frente { self.objetos.len() - 1 } else { 0 };
        if i == destino {
            return false;
        }
        let o = self.objetos.remove(i);
        self.objetos.insert(destino, o);
        true
    }
}

/// Os objetos soltos da aba já lidos (campo inválido = nenhum).
pub fn ler_da_aba(sheet: &Sheet) -> ObjetosAba {
    ObjetosAba::ler(sheet.ob.as_deref()).unwrap_or_default()
}

/// Grava os objetos na aba (um Ctrl+Z). Devolve se algo mudou.
pub fn gravar_na_aba(sheet: &mut Sheet, objetos: &ObjetosAba) -> bool {
    sheet.alterar_objetos(objetos.gravar())
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Colunas de 100 px (a coluna 2 tem 50), linhas de 20 px.
    struct Grade {
        estreita: bool,
    }
    impl Grade {
        fn largura(&self, c: u32) -> f32 {
            if self.estreita && c == 2 { 50.0 } else { 100.0 }
        }
    }
    impl Regua for Grade {
        fn inicio_coluna(&self, c: u32) -> f32 {
            (0..c).map(|i| self.largura(i)).sum()
        }
        fn inicio_linha(&self, r: u32) -> f32 {
            r as f32 * 20.0
        }
        fn coluna_em(&self, x: f32) -> u32 {
            let mut c = 0;
            while self.inicio_coluna(c + 1) <= x {
                c += 1;
            }
            c
        }
        fn linha_em(&self, y: f32) -> u32 {
            (y / 20.0).floor().max(0.0) as u32
        }
    }

    #[test]
    fn ancora_vai_e_volta_pelo_retangulo() {
        let g = Grade { estreita: false };
        let ret = Retangulo::novo(130.0, 45.0, 220.0, 60.0);
        let o = ObjetoSolto::nova_forma(1, "rect", ret, &g);
        assert_eq!(o.de, Ancora { linha: 2, coluna: 1, dx: 30.0, dy: 5.0 });
        assert_eq!(o.ate, Ancora { linha: 5, coluna: 3, dx: 50.0, dy: 5.0 });
        assert_eq!(o.retangulo(&g), ret);
    }

    #[test]
    fn coluna_mais_estreita_no_meio_encolhe_a_forma_e_antes_dela_desloca() {
        let ret = Retangulo::novo(130.0, 45.0, 220.0, 60.0);
        let o = ObjetoSolto::nova_forma(1, "rect", ret, &Grade { estreita: false });
        // A coluna C (2) passou de 100 para 50: a forma (de B a D) encolhe 50.
        let novo = o.retangulo(&Grade { estreita: true });
        assert_eq!((novo.x, novo.largura), (130.0, 170.0));
        // Uma forma que começa depois de C anda 50 para a esquerda.
        let depois = ObjetoSolto::nova_forma(2, "ellipse", Retangulo::novo(320.0, 0.0, 40.0, 40.0), &Grade { estreita: false });
        assert_eq!(depois.retangulo(&Grade { estreita: true }).x, 270.0);
    }

    #[test]
    fn deslocamento_nao_passa_do_fim_da_celula() {
        let o = ObjetoSolto::nova_forma(1, "rect", Retangulo::novo(290.0, 0.0, 5.0, 5.0), &Grade { estreita: false });
        assert_eq!(o.de.coluna, 2);
        assert_eq!(o.de.dx, 90.0);
        // Com C em 50 px, o canto encosta no fim de C (200 + 50).
        assert_eq!(o.de.ponto(&Grade { estreita: true }).0, 250.0);
    }

    #[test]
    fn json_tem_o_formato_documentado_e_preserva_chaves_desconhecidas() {
        let g = Grade { estreita: false };
        let mut aba = ObjetosAba::default();
        let mut o = ObjetoSolto::nova_forma(0, "star5", Retangulo::novo(10.0, 10.0, 80.0, 80.0), &g);
        o.texto = Some("Meta".into());
        o.definir_giro(-30.0);
        assert_eq!(aba.adicionar(o), Some(1));
        let texto = aba.gravar().unwrap();
        let v: Value = serde_json::from_str(&texto).unwrap();
        assert_eq!(v["v"], 1);
        assert_eq!(v["objetos"][0]["tipo"], "forma");
        assert_eq!(v["objetos"][0]["forma"], "star5");
        assert_eq!(v["objetos"][0]["giro"], 330.0);
        assert_eq!(v["objetos"][0]["de"]["coluna"], 0);
        assert_eq!(v["objetos"][0]["aparencia"]["preenchimento"], "#4472c4");
        assert_eq!(ObjetosAba::ler(Some(&texto)).unwrap(), aba);

        let futuro = r##"{"v":1,"objetos":[{"id":7,"tipo":"icone","forma":"calendar","de":{"linha":0,"coluna":0},"ate":{"linha":1,"coluna":1},"ajustes":[0.25],"sombra":{"x":2}}]}"##;
        let lido = ObjetosAba::ler(Some(futuro)).unwrap();
        assert_eq!(lido.objetos[0].tipo, TipoObjeto::Icone);
        let wa = r##"{"v":1,"objetos":[{"id":9,"tipo":"wordart","forma":"arco","de":{"linha":0,"coluna":0},"ate":{"linha":2,"coluna":3},"ajustes":[0.5],"texto":"Vendas","estilo":"dourado","sombra":{"cor":"#20212480","dx":3.0,"dy":3.0}}]}"##;
        let lido_wa = ObjetosAba::ler(Some(wa)).unwrap();
        assert_eq!(lido_wa.objetos[0].tipo, TipoObjeto::Wordart);
        assert_eq!(lido_wa.objetos[0].extras["estilo"], "dourado");
        let volta = lido_wa.gravar().unwrap();
        assert!(volta.contains(r#""tipo":"wordart""#) && volta.contains(r#""sombra""#));
        assert_eq!(lido.objetos[0].aparencia, Aparencia::default());
        let regravado: Value = serde_json::from_str(&lido.gravar().unwrap()).unwrap();
        assert_eq!(regravado["objetos"][0]["ajustes"][0], 0.25);
        assert_eq!(regravado["objetos"][0]["sombra"]["x"], 2);
    }

    #[test]
    fn leitura_recusa_versao_e_lixo_e_vazio_e_sem_objetos() {
        assert!(ObjetosAba::ler(Some(r#"{"v":9,"objetos":[]}"#)).is_err());
        assert!(ObjetosAba::ler(Some("nada")).is_err());
        assert!(ObjetosAba::ler(None).unwrap().objetos.is_empty());
        assert!(ObjetosAba::ler(Some("  ")).unwrap().objetos.is_empty());
        assert_eq!(ObjetosAba::default().gravar(), None);
    }

    #[test]
    fn linha_nasce_sem_preenchimento_e_reordenar_muda_a_pilha() {
        let g = Grade { estreita: false };
        let mut aba = ObjetosAba::default();
        let a = aba.adicionar(ObjetoSolto::nova_forma(0, "line", Retangulo::novo(0.0, 0.0, 50.0, 0.0), &g)).unwrap();
        let b = aba.adicionar(ObjetoSolto::nova_forma(0, "rect", Retangulo::novo(0.0, 0.0, 50.0, 50.0), &g)).unwrap();
        assert!(aba.obter(a).unwrap().aparencia.preenchimento.is_none());
        assert!(aba.reordenar(a, true));
        assert_eq!(aba.objetos.last().unwrap().id, a);
        assert!(!aba.reordenar(a, true));
        assert!(aba.reordenar(a, false));
        assert_eq!(aba.objetos[0].id, a);
        assert_eq!(aba.remover(b).unwrap().id, b);
        assert_eq!(aba.proximo_id(), a + 1);
    }

    #[test]
    fn cada_operacao_e_um_ctrl_z_e_refazer_volta() {
        let g = Grade { estreita: false };
        let mut sheet = Sheet::new("Aba");
        // Criar.
        let mut aba = ler_da_aba(&sheet);
        let id = aba.adicionar(ObjetoSolto::nova_forma(0, "rect", Retangulo::novo(10.0, 10.0, 100.0, 60.0), &g)).unwrap();
        assert!(gravar_na_aba(&mut sheet, &aba));
        let criado = sheet.ob.clone();
        // Mover.
        let mut aba = ler_da_aba(&sheet);
        let o = aba.obter_mut(id).unwrap();
        o.ancorar(Retangulo::novo(200.0, 100.0, 100.0, 60.0), &g);
        assert!(gravar_na_aba(&mut sheet, &aba));
        let movido = sheet.ob.clone();
        // Mudar a aparência.
        let mut aba = ler_da_aba(&sheet);
        aba.obter_mut(id).unwrap().aparencia.preenchimento = Some("#ff0000".into());
        assert!(gravar_na_aba(&mut sheet, &aba));
        // Gravar igual não cria passo.
        let iguais = ler_da_aba(&sheet);
        assert!(!gravar_na_aba(&mut sheet, &iguais));
        // Apagar.
        let mut aba = ler_da_aba(&sheet);
        aba.remover(id);
        assert!(gravar_na_aba(&mut sheet, &aba));
        assert_eq!(sheet.ob, None);

        assert!(sheet.undo());
        assert_eq!(ler_da_aba(&sheet).obter(id).unwrap().aparencia.preenchimento.as_deref(), Some("#ff0000"));
        assert!(sheet.undo());
        assert_eq!(sheet.ob, movido);
        assert!(sheet.undo());
        assert_eq!(sheet.ob, criado);
        assert!(sheet.undo());
        assert_eq!(sheet.ob, None);
        assert!(sheet.redo());
        assert_eq!(sheet.ob, criado);
    }

    #[test]
    fn duplicar_aba_copia_os_objetos() {
        let g = Grade { estreita: false };
        let mut sheet = Sheet::new("Aba");
        let mut aba = ler_da_aba(&sheet);
        aba.adicionar(ObjetoSolto::nova_forma(0, "ellipse", Retangulo::novo(0.0, 0.0, 30.0, 30.0), &g));
        gravar_na_aba(&mut sheet, &aba);
        assert_eq!(sheet.duplicar("Cópia").ob, sheet.ob);
    }

    #[test]
    fn objetos_sobrevivem_ao_fshz_e_o_xlsx_continua_abrindo() {
        let g = Grade { estreita: false };
        let mut aba = ObjetosAba::default();
        let mut seta = ObjetoSolto::nova_forma(0, "rightArrow", Retangulo::novo(130.0, 45.0, 220.0, 60.0), &g);
        seta.definir_giro(15.0);
        seta.texto = Some("Próximo".into());
        aba.adicionar(seta);
        aba.adicionar(ObjetoSolto::nova_forma(0, "line", Retangulo::novo(10.0, 10.0, 90.0, 30.0), &g));
        let mut wb = feathersheet_core::planilha::workbook::Workbook::new();
        assert!(gravar_na_aba(&mut wb.aba_mut(0).unwrap().sheet, &aba));
        let bytes = crate::da_workbook::pasta_gravar_bytes(&wb, 0).unwrap();
        let aberta = crate::para_workbook::para_workbook(&crate::fshz_read::ler_fshz(&bytes).unwrap());
        assert_eq!(ler_da_aba(&aberta.aba(0).unwrap().sheet), aba);
        // Sem exportação de formas para o .xlsx ainda: só não pode quebrar.
        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        assert!(crate::xlsx_read::ler_xlsx_workbook(xlsx).is_ok());
    }

    #[test]
    fn ajustes_e_macro_vao_e_voltam_e_somem_quando_vazios() {
        let g = Grade { estreita: false };
        let mut o = ObjetoSolto::nova_forma(1, "roundRect", Retangulo::novo(0.0, 0.0, 100.0, 50.0), &g);
        let texto = serde_json::to_string(&o).unwrap();
        assert!(!texto.contains("ajustes") && !texto.contains("macro"), "{texto}");
        o.ajustes = vec![0.1, 0.2, 0.3, 0.4];
        o.macro_nome = Some("Relatorio".into());
        let v: Value = serde_json::to_value(&o).unwrap();
        assert_eq!(v["macro"], "Relatorio");
        assert_eq!(v["ajustes"][3], 0.4f32 as f64);
        let volta: ObjetoSolto = serde_json::from_value(v).unwrap();
        assert_eq!(volta, o);
    }
}
