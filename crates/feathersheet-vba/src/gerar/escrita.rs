//! Escrever no Excel: `obj.prop = valor` e `obj.Metodo args`, na API das macros. Cada
//! função devolve a instrução JS pronta (ou vazia, quando o VBA pedia algo que não tem
//! efeito aqui, como `ScreenUpdating`).

use super::contexto::Gerador;
use super::funcoes::valor_da_constante;
use super::tipos::{Faixa, Inteira, Motivo, Obj, Parte, R, T, V};
use crate::sintaxe::{Arg, Expr};

/// Propriedades que só mexem no visual de um jeito que a planilha não tem (tom, padrão,
/// espessura da borda): aceitas e ignoradas, sem virar pendência.
const SEM_EFEITO: [&str; 10] = ["tintandshade", "themecolor", "pattern", "patterncolorindex", "patterncolor", "patterntintandshade", "weight", "themefont", "fontstyle", "indentlevel"];

/// A constante na expressão, por nome.
fn constante(e: &Expr) -> Option<i64> {
    match e {
        Expr::Nome(n) => valor_da_constante(n),
        Expr::Num(x) => Some(*x as i64),
        Expr::Neg(x) => constante(x).map(|v| -v),
        _ => None,
    }
}

impl Gerador<'_> {
    /// `obj.prop = valor` → a instrução JS.
    pub fn atribuir_propriedade(&mut self, o: Obj, prop: &str, valor: &Expr) -> R<String> {
        let n = prop.to_lowercase();
        if SEM_EFEITO.contains(&n.as_str()) {
            return Ok(String::new());
        }
        if n == "onaction" {
            return Err(Motivo::novo("formas e botões do Excel (use Inserir → Botão de macro)"));
        }
        if let Some((o, end)) = por_area(&o) {
            let js = self.atribuir_propriedade(o, prop, valor)?;
            return Ok(format!("for (const $area of String({end}).split(',')) {{ {js} }}"));
        }
        match o {
            Obj::Faixa(f) => self.escrever_na_faixa(&f, &n, valor),
            Obj::Parte(f, Parte::Interior) => self.escrever_fundo(&f, &n, valor),
            Obj::Parte(f, Parte::Fonte) => self.escrever_fonte(&f, &n, valor),
            Obj::Parte(f, Parte::Bordas(lado)) => self.escrever_borda(&f, lado.as_deref(), &n, valor),
            Obj::App => self.escrever_no_app(&n, valor),
            Obj::Controle(c) => self.escrever_no_controle(&c, &n, valor),
            Obj::Formulario(f) => match self.p.controle(&f, prop).map(String::from) {
                Some(c) => self.escrever_no_controle(&c, "value", valor),
                None => Err(Motivo::novo(format!("{f}.{prop} do formulário"))),
            },
            Obj::Aba(aba, nome) => self.escrever_na_aba(&aba, nome, &n, valor),
            Obj::Janela => self.escrever_na_janela(&n),
            Obj::Ordem(aba) => match n.as_str() {
                "header" => {
                    let v = self.valor(valor)?;
                    Ok(format!("vba.{}({aba}).cabecalho = {};", self.ajuda("ordem"), v.js))
                }
                // Maiúsculas, método (PinYin/traço) e orientação de cima a baixo: o padrão da API.
                "matchcase" | "sortmethod" => Ok(String::new()),
                "orientation" if constante(valor) == Some(1) => Ok(String::new()),
                _ => Err(Motivo::novo(format!("Sort.{prop}"))),
            },
            // Mensagens, ícone e seta da lista: a validação da API não guarda esses detalhes.
            Obj::EstiloCondicao(c, parte) => {
                let v = self.valor(valor)?;
                let campo = match (&parte, n.as_str()) {
                    (Parte::Interior, "color") => "fill",
                    (Parte::Fonte, "color") => "color",
                    (Parte::Fonte, "bold") => "bold",
                    (Parte::Fonte, "italic") => "italic",
                    (Parte::Interior | Parte::Fonte, "colorindex") => {
                        return Ok(format!("{c}.o.{} = vba.{}({});", if parte == Parte::Interior { "fill" } else { "color" }, self.ajuda("corIndice"), v.js));
                    }
                    _ => return Err(Motivo::novo(format!("estilo da formatação condicional: {prop}"))),
                };
                Ok(format!("{c}.o.{campo} = {};", v.js))
            }
            // Parar se verdadeiro e prioridade: a planilha aplica todas as regras em ordem.
            Obj::Condicao(_) if matches!(n.as_str(), "stopiftrue" | "priority") => Ok(String::new()),
            Obj::Validacao(_) if matches!(n.as_str(), "ignoreblank" | "incelldropdown" | "inputtitle" | "errortitle" | "inputmessage" | "errormessage" | "showinput" | "showerror" | "imemode") => Ok(String::new()),
            _ => Err(Motivo::novo(format!("escrever {prop}"))),
        }
    }

    fn escrever_na_faixa(&mut self, f: &Faixa, n: &str, valor: &Expr) -> R<String> {
        let (aba, end) = (&f.aba, &f.end);
        let v = self.valor(valor)?;
        Ok(match n {
            "value" | "value2" => {
                if e_data(valor) {
                    // A célula reconhece a data digitada e ganha o formato de data, como no Excel.
                    format!("{aba}.set({end}, vba.{}({}));", self.ajuda("dataTexto"), v.js)
                } else if f.uma_celula() || matches!(v.t, T::Num | T::Texto | T::Bool) {
                    format!("{aba}.set({end}, {});", v.js)
                } else {
                    // Matriz (ou Variant que pode ser): tira o índice que o VBA não usa e recorta à área.
                    format!("vba.{}({aba}, {end}, {});", self.ajuda("escrever"), v.js)
                }
            }
            "formula" | "formulalocal" => format!("{aba}.setFormula({end}, {});", formula(valor, &v)),
            "formular1c1" => format!("{aba}.setFormulaR1C1({end}, {});", formula(valor, &v)),
            "numberformat" | "numberformatlocal" => format!("{aba}.setNumberFormat({end}, {});", v.js),
            "wraptext" => format!("{aba}.setWrapText({end}, {});", self.como_bool(&v)),
            "mergecells" => match valor {
                Expr::Bool(true) => format!("vba.{}({aba}, {end});", self.ajuda("mesclar")),
                Expr::Bool(false) => format!("{aba}.unmerge({end});"),
                _ => format!("if ({}) {aba}.merge({end}); else {aba}.unmerge({end});", self.como_bool(&v)),
            },
            "horizontalalignment" => format!("{aba}.setAlignHorizontal({end}, {});", self.alinhamento(valor, &v, "alinH")),
            "verticalalignment" => format!("{aba}.setAlignVertical({end}, {});", self.alinhamento(valor, &v, "alinV")),
            "rowheight" => {
                let l = self.ajuda("linha");
                format!("{aba}.setRowHeight(vba.{l}({end}), Math.round({} * 4 / 3));", self.como_num(&v))
            }
            "columnwidth" => {
                let c = self.ajuda("coluna");
                format!("{aba}.setColumnWidth(vba.{c}({end}), Math.round({} * 7 + 5));", self.como_num(&v))
            }
            "hidden" => self.esconder(f, &v)?,
            "orientation" => format!("{aba}.setTextRotation({end}, {});", self.como_num(&v)),
            _ => return Err(Motivo::novo(format!("escrever Range.{n}"))),
        })
    }

    fn alinhamento(&mut self, e: &Expr, v: &V, ajuda: &'static str) -> String {
        let fixo = constante(e).map(|c| match (ajuda, c) {
            ("alinH", -4131) => "'left'",
            ("alinH", -4108 | 7) => "'center'",
            ("alinH", -4152) => "'right'",
            ("alinV", -4160) => "'top'",
            ("alinV", -4108) => "'middle'",
            ("alinV", _) => "'bottom'",
            _ => "'general'",
        });
        match fixo {
            Some(f) => f.into(),
            None => format!("vba.{}({})", self.ajuda(ajuda), v.js),
        }
    }

    fn esconder(&mut self, f: &Faixa, v: &V) -> R<String> {
        let b = self.como_bool(v);
        Ok(match f.inteira {
            Inteira::Linhas => format!("{}.hideRow(vba.{}({}), {b});", f.aba, self.ajuda("linha"), f.end),
            Inteira::Colunas => format!("{}.hideColumn(vba.{}({}), {b});", f.aba, self.ajuda("coluna"), f.end),
            Inteira::Nao => return Err(Motivo::novo("esconder célula que não é linha/coluna inteira")),
        })
    }

    fn escrever_fundo(&mut self, f: &Faixa, n: &str, valor: &Expr) -> R<String> {
        if constante(valor) == Some(-4142) {
            return Ok(format!("vba.{}({}, {});", self.ajuda("semFundo"), f.aba, f.end));
        }
        let v = self.valor(valor)?;
        Ok(match n {
            "color" => format!("{}.setBackground({}, {});", f.aba, f.end, v.js),
            "colorindex" => format!("{}.setBackground({}, vba.{}({}));", f.aba, f.end, self.ajuda("corIndice"), v.js),
            _ => return Err(Motivo::novo(format!("Interior.{n}"))),
        })
    }

    fn escrever_fonte(&mut self, f: &Faixa, n: &str, valor: &Expr) -> R<String> {
        let v = self.valor(valor)?;
        let (aba, end) = (&f.aba, &f.end);
        Ok(match n {
            "bold" => format!("{aba}.setBold({end}, {});", self.como_bool(&v)),
            "italic" => format!("{aba}.setItalic({end}, {});", self.como_bool(&v)),
            "strikethrough" => format!("{aba}.setStrikethrough({end}, {});", self.como_bool(&v)),
            "underline" => {
                let ligado = match constante(valor) {
                    Some(-4142 | 0) => "false".to_string(),
                    Some(_) => "true".to_string(),
                    None if matches!(valor, Expr::Bool(_)) => v.js.clone(),
                    None => format!("![-4142, 0, false].includes({})", v.js),
                };
                format!("{aba}.setUnderline({end}, {ligado});")
            }
            "color" => format!("{aba}.setFontColor({end}, {});", v.js),
            "colorindex" if constante(valor) == Some(-4105) => format!("{aba}.setFontColor({end}, '#000000');"),
            "colorindex" => format!("{aba}.setFontColor({end}, vba.{}({}));", self.ajuda("corIndice"), v.js),
            // O Excel mede a letra em pontos; a API, em pixels.
            "size" => format!("{aba}.setFontSize({end}, Math.round({} * 4 / 3));", self.como_num(&v)),
            "name" => format!("{aba}.setFontFamily({end}, {});", v.js),
            _ => return Err(Motivo::novo(format!("Font.{n}"))),
        })
    }

    fn escrever_borda(&mut self, f: &Faixa, lado: Option<&str>, n: &str, valor: &Expr) -> R<String> {
        if lado == Some("diagonal") {
            return Ok(String::new());
        }
        let lado = lado.map_or("all".to_string(), String::from);
        Ok(match n {
            "linestyle" if constante(valor) == Some(-4142) => format!("{}.setBorder({}, {{ none: true }});", f.aba, f.end),
            "linestyle" => format!("{}.setBorder({}, {{ {lado}: true }});", f.aba, f.end),
            "color" => {
                let v = self.valor(valor)?;
                format!("{}.setBorder({}, {{ {lado}: true, color: {} }});", f.aba, f.end, v.js)
            }
            "colorindex" if matches!(constante(valor), Some(-4105 | 0)) => String::new(),
            "colorindex" => {
                let v = self.valor(valor)?;
                format!("{}.setBorder({}, {{ {lado}: true, color: vba.{}({}) }});", f.aba, f.end, self.ajuda("corIndice"), v.js)
            }
            _ => return Err(Motivo::novo(format!("Borders.{n}"))),
        })
    }

    fn escrever_no_app(&mut self, n: &str, valor: &Expr) -> R<String> {
        Ok(match n {
            "enableevents" => {
                let v = self.valor(valor)?;
                format!("app.setEventsEnabled({});", self.como_bool(&v))
            }
            "statusbar" => {
                let v = self.valor(valor)?;
                format!("app.status({});", v.js)
            }
            // A planilha não tem esses interruptores: a tela já não pisca durante a macro.
            "screenupdating" | "displayalerts" | "cutcopymode" | "calculation" | "displaystatusbar" | "enablecancelkey" | "asktoupdatelinks" | "displayformulabar" | "displayfullscreen" | "displayscrollbars" | "displayheadings" | "displaygridlines" => String::new(),
            _ => return Err(Motivo::novo(format!("Application.{n}"))),
        })
    }

    /// `ActiveWindow.ScrollRow = 1`, `.Zoom = 85`: a tela da macro não rola nem muda o zoom.
    fn escrever_na_janela(&mut self, n: &str) -> R<String> {
        match n {
            "freezepanes" | "splitrow" | "splitcolumn" | "split" => Err(Motivo::novo("congelar painéis pela macro (sem API de macro)")),
            "windowstate" | "displaygridlines" | "displayheadings" | "displayworkbooktabs" => Err(Motivo::novo(format!("ActiveWindow.{n}"))),
            _ => Ok(String::new()),
        }
    }

    /// O argumento pelo nome (`Key:=`) ou pela posição.
    fn arg_de(args: &[Arg], nomes: &[&str], pos: usize) -> Option<Expr> {
        args.iter().find(|a| a.nome.as_deref().is_some_and(|n| nomes.iter().any(|c| n.eq_ignore_ascii_case(c))))
            .or_else(|| args.get(pos).filter(|a| a.nome.is_none()))
            .and_then(|a| a.valor.clone())
    }

    /// O valor JS de um argumento, ou `undefined` quando falta.
    fn arg_js(&mut self, args: &[Arg], nomes: &[&str], pos: usize) -> R<String> {
        Self::arg_de(args, nomes, pos).map_or(Ok("undefined".into()), |e| self.valor(&e).map(|v| v.js))
    }

    /// O endereço JS de um argumento que é intervalo (`Key:=Range("B1:B9")`).
    fn arg_faixa(&mut self, args: &[Arg], nomes: &[&str], pos: usize) -> R<Option<String>> {
        let Some(e) = Self::arg_de(args, nomes, pos) else { return Ok(None) };
        Ok(Some(match self.objeto(&e) {
            Ok(Obj::Faixa(f)) => f.end,
            _ => self.valor(&e)?.js,
        }))
    }

    /// `Worksheet.Sort`: `.SortFields.Clear/Add`, `.SetRange`, `.Apply`.
    fn metodo_da_ordem(&mut self, aba: &str, n: &str, args: &[Arg]) -> R<String> {
        let o = self.ajuda("ordem");
        Ok(match n {
            "clear" => format!("vba.{o}({aba}).chaves = [];"),
            "add" | "add2" => {
                let chave = self.arg_faixa(args, &["key"], 0)?.ok_or_else(|| Motivo::novo("SortFields.Add sem Key"))?;
                let ordem = self.arg_js(args, &["order"], 2)?;
                format!("vba.{o}({aba}).chaves.push([{chave}, {ordem}]);")
            }
            "setrange" => {
                let f = self.arg_faixa(args, &["rng"], 0)?.ok_or_else(|| Motivo::novo("SetRange sem intervalo"))?;
                format!("vba.{o}({aba}).faixa = {f};")
            }
            "apply" => format!("vba.{}({aba});", self.ajuda("ordenar")),
            _ => return Err(Motivo::novo(format!("Sort.{n}"))),
        })
    }

    /// `Range.Sort Key1:=…, Order1:=…, Header:=…` (o jeito antigo, até três chaves).
    fn ordenar_faixa(&mut self, f: &Faixa, args: &[Arg]) -> R<String> {
        let mut chaves = Vec::new();
        for (k, ord, pk, po) in [("key1", "order1", 0, 1), ("key2", "order2", 3, 4), ("key3", "order3", 5, 6)] {
            if let Some(c) = self.arg_faixa(args, &[k], pk)? {
                let o = self.arg_js(args, &[ord], po)?;
                chaves.push(format!("[{c}, {o}]"));
            }
        }
        if chaves.is_empty() {
            return Err(Motivo::novo("Range.Sort sem Key1"));
        }
        let cab = self.arg_js(args, &["header"], 7)?;
        Ok(format!("vba.{}({}, {}, [{}], {cab});", self.ajuda("ordenarFaixa"), f.aba, f.end, chaves.join(", ")))
    }

    /// `Range.Validation`: `.Delete` e `.Add Type, AlertStyle, Operator, Formula1, Formula2`.
    fn metodo_da_validacao(&mut self, f: &Faixa, n: &str, args: &[Arg]) -> R<String> {
        Ok(match n {
            "delete" => format!("{}.clearDataValidation({});", f.aba, f.end),
            "add" | "modify" => {
                let mut p = Vec::new();
                for (nomes, pos) in [(&["type"][..], 0), (&["alertstyle"][..], 1), (&["operator"][..], 2), (&["formula1"][..], 3), (&["formula2"][..], 4)] {
                    p.push(self.arg_js(args, nomes, pos)?);
                }
                format!("vba.{}({}, {}, {});", self.ajuda("validar"), f.aba, f.end, p.join(", "))
            }
            _ => return Err(Motivo::novo(format!("Validation.{n}"))),
        })
    }

    /// `Range.TextToColumns` delimitado → `aba.textToColumns(end, {…})`.
    /// FieldInfo (tipo por coluna) não entra: o motor detecta número/data
    /// como o assistente da faixa. Largura fixa fica marcada.
    fn texto_para_colunas(&mut self, f: &Faixa, args: &[Arg]) -> R<String> {
        let sim = |e: Option<Expr>| -> R<Option<bool>> {
            Ok(match e {
                None => None,
                Some(Expr::Bool(b)) => Some(b),
                Some(e) => match constante(&e) {
                    Some(n) => Some(n != 0),
                    None => return Err(Motivo::novo("TextToColumns com opção calculada (use True/False)")),
                },
            })
        };
        if let Some(t) = Self::arg_de(args, &["datatype"], 1) {
            if constante(&t) == Some(2) {
                return Err(Motivo::novo("Texto para colunas com largura fixa (xlFixedWidth)"));
            }
        }
        let mut delims: Vec<String> = Vec::new();
        for (nome, pos, js) in [("tab", 4, "'\\t'"), ("semicolon", 5, "';'"), ("comma", 6, "','"), ("space", 7, "' '")] {
            if sim(Self::arg_de(args, &[nome], pos))? == Some(true) { delims.push(js.into()); }
        }
        if sim(Self::arg_de(args, &["other"], 8))? == Some(true) {
            let c = Self::arg_de(args, &["otherchar"], 9).ok_or_else(|| Motivo::novo("TextToColumns Other:=True sem OtherChar"))?;
            let v = self.valor(&c)?;
            delims.push(format!("String({}).charAt(0)", v.js));
        }
        if delims.is_empty() {
            // Sem delimitador marcado o Excel usa o último escolhido no assistente: tab.
            delims.push("'\\t'".into());
        }
        let mut opcoes = vec![format!("delimiters: [{}]", delims.join(", "))];
        if sim(Self::arg_de(args, &["consecutivedelimiter"], 3))? == Some(true) {
            opcoes.push("consecutive: true".into());
        }
        match Self::arg_de(args, &["textqualifier"], 2).as_ref().and_then(constante) {
            Some(2) => opcoes.push("qualifier: \"'\"".into()),
            Some(-4142) => opcoes.push("qualifier: ''".into()),
            _ => {}
        }
        if let Some(d) = self.arg_faixa(args, &["destination"], 0)? {
            if d != f.end { opcoes.push(format!("destination: {d}")); }
        }
        Ok(format!("{}.textToColumns({}, {{ {} }});", f.aba, f.end, opcoes.join(", ")))
    }

    /// `ExportAsFixedFormat Type:=xlTypePDF, Filename:=…` → `aba.exportPdf(caminho)`.
    /// Qualidade, páginas e abrir depois de publicar não mudam o arquivo daqui.
    fn exportar_pdf(&mut self, aba: &str, args: &[Arg]) -> R<String> {
        if let Some(t) = Self::arg_de(args, &["type"], 0) {
            if constante(&t) != Some(0) {
                return Err(Motivo::novo("exportar em formato que não é PDF (XPS)"));
            }
        }
        let caminho = Self::arg_de(args, &["filename"], 1).ok_or_else(|| Motivo::novo("ExportAsFixedFormat sem Filename"))?;
        let v = self.valor(&caminho)?;
        Ok(format!("{aba}.exportPdf({});", self.como_texto(&v)))
    }

    /// `Sheet7.Copy Before:=newBook.Sheets(1)`: a aba entra na lista da pasta nova.
    /// Sem destino o Excel cria uma pasta e a deixa ativa: isso não converte.
    fn copiar_aba(&mut self, nome_js: &str, args: &[Arg]) -> R<String> {
        for (chave, pos, desloca) in [("before", 0, " - 1"), ("after", 1, "")] {
            let Some(e) = Self::arg_de(args, &[chave], pos) else { continue };
            return match self.objeto(&e)? {
                Obj::AbaNova(p, i) => Ok(format!("{p}.abas.splice({i}{desloca}, 0, {nome_js});")),
                _ => Err(Motivo::novo("copiar a aba dentro da mesma pasta (use wb.copySheet)")),
            };
        }
        Err(Motivo::novo("copiar a aba para uma pasta nova sem Workbooks.Add"))
    }

    /// `newBook.SaveAs Filename` grava as abas copiadas num arquivo novo; `Close` não
    /// tem o que fechar (a pasta nova nunca abre numa janela).
    fn metodo_da_nova_pasta(&mut self, p: &str, n: &str, args: &[Arg]) -> R<String> {
        match n {
            "saveas" | "savecopyas" => {
                let caminho = Self::arg_de(args, &["filename"], 0).ok_or_else(|| Motivo::novo("SaveAs sem nome de arquivo"))?;
                let v = self.valor(&caminho)?;
                Ok(format!("wb.saveSheetsAs({p}.abas.filter((a) => a !== null), {});", self.como_texto(&v)))
            }
            "close" => Ok(String::new()),
            _ => Err(Motivo::novo(format!("Workbook.{n} da pasta nova"))),
        }
    }

    fn escrever_na_aba(&mut self, aba: &str, nome: Option<String>, n: &str, valor: &Expr) -> R<String> {
        let atual = nome.map_or_else(|| format!("{aba}.name"), |x| format!("{x:?}"));
        let v = self.valor(valor)?;
        Ok(match n {
            "name" => format!("wb.renameSheet({atual}, {});", v.js),
            "visible" => {
                let visivel = match constante(valor) {
                    Some(0 | 2) => "false".to_string(),
                    Some(_) => "true".to_string(),
                    None => self.como_bool(&v),
                };
                format!("wb.setSheetVisible({atual}, {visivel});")
            }
            _ => return Err(Motivo::novo(format!("escrever Worksheet.{n}"))),
        })
    }

    /// `TextBox1.Value = x` → `sheet.field("TextBox1").value = x;`.
    pub fn escrever_no_controle(&mut self, c: &str, n: &str, valor: &Expr) -> R<String> {
        let p = match n {
            "value" | "text" => "value",
            "caption" => "caption",
            "enabled" => "enabled",
            "visible" => "visible",
            _ => return Err(Motivo::novo(format!("{c}.{n} do controle do formulário"))),
        };
        let v = self.valor(valor)?;
        Ok(format!("sheet.field({c:?}).{p} = {};", v.js))
    }

    // ------------------------------------------------------------------ métodos

    /// `obj.Metodo args` → a instrução JS.
    pub fn metodo(&mut self, o: Obj, nome: &str, args: &[Arg]) -> R<String> {
        let n = nome.to_lowercase();
        if let Some((o, end)) = por_area(&o) {
            let js = self.metodo(o, nome, args)?;
            return Ok(format!("for (const $area of String({end}).split(',')) {{ {js} }}"));
        }
        match o {
            Obj::Faixa(f) => self.metodo_da_faixa(&f, &n, args),
            Obj::Formulario(f) => match n.as_str() {
                "show" => Ok(format!("sheet.showComponent({f:?});")),
                "hide" => Ok("sheet.hideComponent();".into()),
                _ => Err(Motivo::novo(format!("{f}.{nome} do formulário"))),
            },
            Obj::Controle(c) => match n.as_str() {
                "setfocus" => Ok(String::new()),
                _ => Err(Motivo::novo(format!("{c}.{nome}: a lista do formulário mudada pela macro (só AddItem com texto fixo no UserForm_Initialize converte)"))),
            },
            Obj::Aba(aba, nome_aba) => self.metodo_da_aba(&aba, nome_aba, &n, args),
            Obj::Nomes if n == "add" => self.nome_novo(args),
            Obj::Abas if n == "add" => Ok(format!("{};", self.aba_nova_da_pasta(args)?)),
            // Rolar, ativar outra janela, zoom: a planilha da macro não tem janela.
            Obj::Janela if n == "close" => Err(Motivo::novo("fechar a janela do Excel")),
            Obj::Janela => Ok(String::new()),
            Obj::Ordem(aba) => self.metodo_da_ordem(&aba, &n, args),
            Obj::Validacao(f) => self.metodo_da_validacao(&f, &n, args),
            Obj::Condicoes(f) if n == "delete" => Ok(format!("vba.{}({}, {});", self.ajuda("condApagar"), f.aba, f.end)),
            Obj::Condicoes(f) if n == "add" => {
                let c = self.condicao_nova(&f, args)?;
                Ok(format!("{c}; vba.{}();", self.ajuda("condAplicar")))
            }
            Obj::NovaPasta(p) => self.metodo_da_nova_pasta(&p, &n, args),
            Obj::Pastas if n == "open" => Err(Motivo::novo("abrir outra pasta de trabalho (Workbooks.Open)")),
            Obj::Pastas => Err(Motivo::novo(format!("Workbooks.{nome} (só Set x = Workbooks.Add converte)"))),
            Obj::AbaNova(p, i) if n == "delete" => Ok(format!("{p}.abas.splice({i} - 1, 1);")),
            Obj::App | Obj::Pasta => match n.as_str() {
                "calculate" | "calculatefull" => Ok("app.calculate();".into()),
                // Função que recalcula sempre: a planilha recalcula o que depende do que mudou.
                "volatile" => Ok(String::new()),
                "save" | "saveas" | "close" | "savecopyas" => Err(Motivo::novo("salvar/fechar o arquivo pela macro")),
                "run" | "ontime" | "wait" | "quit" | "goto" | "sendkeys" => Err(Motivo::novo(format!("Application.{nome}"))),
                _ => Err(Motivo::novo(format!("{nome} da pasta/Application"))),
            },
            _ => Err(Motivo::novo(format!("método {nome}"))),
        }
    }

    fn metodo_da_faixa(&mut self, f: &Faixa, n: &str, args: &[Arg]) -> R<String> {
        let (aba, end) = (&f.aba, &f.end);
        Ok(match n {
            "select" | "activate" => format!("{aba}.select({end});"),
            "clearcontents" => format!("{aba}.clearContents({end});"),
            "clear" => format!("{aba}.clear({end});"),
            "clearformats" => format!("{aba}.clearFormats({end});"),
            "merge" => format!("vba.{}({aba}, {end});", self.ajuda("mesclar")),
            "unmerge" => format!("{aba}.unmerge({end});"),
            "delete" | "insert" => self.inserir_excluir(f, n == "delete", args)?,
            "sort" => self.ordenar_faixa(f, args)?,
            "copy" => self.copiar(f, args, false)?,
            "cut" => self.copiar(f, args, true)?,
            "pastespecial" => {
                let modo = match Self::arg_de(args, &["paste"], 0) {
                    None => "'all'".to_string(),
                    Some(e) => match constante(&e) {
                        Some(-4163 | 12) => "'values'".into(),
                        Some(-4122) => "'formats'".into(),
                        Some(-4123 | 11) => "'formulas'".into(),
                        Some(-4104) => "'all'".into(),
                        Some(_) => return Err(Motivo::novo("colar especial (largura de coluna, comentários, validação)")),
                        None => {
                            let v = self.valor(&e)?;
                            format!("({{ [-4163]: 'values', [-4122]: 'formats', [-4123]: 'formulas' }}[{}] ?? 'all')", v.js)
                        }
                    },
                };
                format!("vba.{}({aba}, {end}, {modo});", self.ajuda("colar"))
            }
            "autofit" if f.inteira == Inteira::Colunas => format!("{aba}.autoFitColumns({end});"),
            "autofit" => String::new(),
            "replace" => {
                let p: Vec<&Expr> = args.iter().filter_map(|a| a.valor.as_ref()).collect();
                let (Some(a), Some(b)) = (p.first(), p.get(1)) else { return Err(Motivo::novo("Replace sem argumentos")) };
                let (va, vb) = (self.valor(a)?, self.valor(b)?);
                format!("{aba}.replace({end}, {}, {});", va.js, vb.js)
            }
            "copypicture" => return Err(Motivo::novo("copiar como imagem (CopyPicture)")),
            "texttocolumns" => self.texto_para_colunas(f, args)?,
            "removeduplicates" => {
                let cols = self.arg_js(args, &["columns"], 0)?;
                let cab = self.arg_js(args, &["header"], 1)?;
                format!("vba.{}({aba}, {end}, {cols}, {cab});", self.ajuda("repetidas"))
            }
            _ => return Err(Motivo::novo(format!("Range.{n}"))),
        })
    }

    /// `FormatConditions.Add Type, Operator, Formula1, Formula2` → `vba.condNova(…)` (o JS da regra).
    pub fn condicao_nova(&mut self, f: &Faixa, args: &[Arg]) -> R<String> {
        let tipo = self.arg_js(args, &["type"], 0)?;
        if !matches!(constante(&Self::arg_de(args, &["type"], 0).unwrap_or(Expr::Num(1.0))), Some(1 | 2)) {
            return Err(Motivo::novo("formatação condicional que não é por valor nem por fórmula (escala, barra, ícones: há sheet.addColorScale/addDataBar)"));
        }
        let op = self.arg_js(args, &["operator"], 1)?;
        let mut fs = Vec::new();
        for (nome, pos) in [("formula1", 2), ("formula2", 3)] {
            fs.push(match Self::arg_de(args, &[nome], pos) {
                Some(e) => {
                    let v = self.valor(&e)?;
                    formula(&e, &v)
                }
                None => "undefined".into(),
            });
        }
        Ok(format!("vba.{}({}, {}, {tipo}, {op}, {})", self.ajuda("condNova"), f.aba, f.end, fs.join(", ")))
    }

    /// `Names.Add Name:=…, RefersTo:=Range(…)` → `wb.addName(nome, endereço)`.
    fn nome_novo(&mut self, args: &[Arg]) -> R<String> {
        let achar = |chaves: &[&str], pos: usize| args.iter().find(|a| a.nome.as_deref().is_some_and(|n| chaves.iter().any(|c| n.eq_ignore_ascii_case(c)))).or_else(|| args.get(pos).filter(|a| a.nome.is_none())).and_then(|a| a.valor.clone());
        let (Some(nome), Some(alvo)) = (achar(&["name"], 0), achar(&["refersto", "referstor1c1", "referstolocal"], 1)) else { return Err(Motivo::novo("Names.Add sem nome ou intervalo")) };
        let n = self.valor(&nome)?;
        let end = match self.objeto(&alvo) {
            Ok(Obj::Faixa(f)) => f.end,
            _ => self.valor(&alvo)?.js,
        };
        Ok(format!("wb.addName({}, {end});", self.como_texto(&n)))
    }

    fn inserir_excluir(&mut self, f: &Faixa, excluir: bool, args: &[Arg]) -> R<String> {
        let (eixo, ajuda_pos, ajuda_qtd) = match f.inteira {
            Inteira::Linhas => ("Row", "linha", "qtdLinhas"),
            Inteira::Colunas => ("Column", "coluna", "qtdColunas"),
            Inteira::Nao => return self.deslocar(f, excluir, args),
        };
        let verbo = if excluir { "delete" } else { "insert" };
        let (p, q) = (self.ajuda(ajuda_pos), self.ajuda(ajuda_qtd));
        Ok(format!("{}.{verbo}{eixo}(vba.{p}({}), vba.{q}({}));", f.aba, f.end, f.end))
    }

    /// `Selection.Delete Shift:=xlUp`, `.Insert Shift:=xlDown`: só as células da seleção
    /// andam. Sem `Shift`, o Excel decide pela forma: mais larga que alta anda na vertical.
    fn deslocar(&mut self, f: &Faixa, excluir: bool, args: &[Arg]) -> R<String> {
        let verbo = if excluir { "deleteCells" } else { "insertCells" };
        let (vertical, horizontal) = if excluir { ("'up'", "'left'") } else { ("'down'", "'right'") };
        let sentido = match Self::arg_de(args, &["shift"], 0) {
            Some(e) => match constante(&e) {
                Some(-4162 | -4121) => vertical.to_string(),
                Some(-4159 | -4161) => horizontal.to_string(),
                _ => {
                    let v = self.valor(&e)?;
                    format!("([-4159, -4161].includes({}) ? {horizontal} : {vertical})", v.js)
                }
            },
            None => {
                let (l, c) = (self.ajuda("qtdLinhas"), self.ajuda("qtdColunas"));
                format!("(vba.{c}({0}) >= vba.{l}({0}) ? {vertical} : {horizontal})", f.end)
            }
        };
        Ok(format!("{}.{verbo}({}, {sentido});", f.aba, f.end))
    }

    /// `.Copy [Destination:=…]` e `.Cut [Destination:=…]`. Sem destino, guarda a
    /// origem para o `Paste` seguinte (a "área de transferência" da macro).
    fn copiar(&mut self, f: &Faixa, args: &[Arg], cortar: bool) -> R<String> {
        let destino = Self::arg_de(args, &["destination"], 0);
        let Some(e) = destino else {
            return Ok(format!("vba.{}({}, {}, {cortar});", self.ajuda("copiar"), f.aba, f.end));
        };
        let Obj::Faixa(d) = self.objeto(&e)? else { return Err(Motivo::novo("destino da cópia")) };
        let mesma = d.aba == f.aba;
        Ok(match (cortar, mesma) {
            (false, true) => format!("{}.copy({}, {});", f.aba, f.end, d.end),
            (false, false) => format!("{}.copy({}, {}, {{ sheet: {}.name }});", f.aba, f.end, d.end, d.aba),
            (true, true) => format!("{}.move({}, {});", f.aba, f.end, d.end),
            (true, false) => format!("{0}.copy({1}, {2}, {{ sheet: {3}.name }}); {0}.clear({1});", f.aba, f.end, d.end, d.aba),
        })
    }

    fn metodo_da_aba(&mut self, aba: &str, nome: Option<String>, n: &str, args: &[Arg]) -> R<String> {
        let nome_js = nome.clone().map_or_else(|| format!("{aba}.name"), |x| format!("{x:?}"));
        Ok(match n {
            "select" | "activate" => {
                self.atual = aba.to_string();
                self.atual_nome = nome;
                format!("wb.activate({nome_js});")
            }
            "calculate" => "app.calculate();".into(),
            "delete" => format!("wb.deleteSheet({nome_js});"),
            "exportasfixedformat" => self.exportar_pdf(aba, args)?,
            "printout" | "printpreview" => return Err(Motivo::novo("imprimir pela macro (use exportar PDF)")),
            "protect" | "unprotect" => return Err(Motivo::novo("proteger a aba (sem API de macro)")),
            "copy" => self.copiar_aba(&nome_js, args)?,
            "move" => return Err(Motivo::novo("mover a aba para outra pasta")),
            // `ActiveSheet.Paste [Destination:=…]`: cola o último `Copy`/`Cut` na seleção.
            "paste" => {
                let alvo = self.arg_faixa(args, &["destination"], 0)?.unwrap_or_else(|| format!("{aba}.selection().address"));
                format!("vba.{}({aba}, {alvo});", self.ajuda("colar"))
            }
            "pastespecial" => return Err(Motivo::novo("colar especial da aba (imagem, link)")),
            _ => return Err(Motivo::novo(format!("Worksheet.{n}"))),
        })
    }
}

/// A fórmula em inglês do VBA (`=ROUND(A1,2)`) com o `;` que a planilha usa para separar
/// argumentos (a vírgula entre algarismos aqui é decimal). Só a fórmula escrita direto no
/// código; a montada em tempo de execução vai como está.
fn formula(e: &Expr, v: &V) -> String {
    let Expr::Texto(t) = e else { return v.js.clone() };
    let mut saida = String::new();
    let mut aspas = false;
    for c in t.chars() {
        match c {
            '"' => {
                aspas = !aspas;
                saida.push(c);
            }
            ',' if !aspas => saida.push(';'),
            _ => saida.push(c),
        }
    }
    format!("{saida:?}")
}

/// `Date`, `Now`, `DateSerial(…)`, `#1/31/2024#`: o valor é uma data com certeza.
fn e_data(e: &Expr) -> bool {
    const DATAS: &[&str] = &["date", "now", "dateserial", "datevalue", "cdate", "dateadd"];
    match e {
        Expr::Data(_) => true,
        Expr::Nome(n) => DATAS.contains(&n.to_lowercase().as_str()),
        Expr::Chamada(f, _) => matches!(&**f, Expr::Nome(n) if DATAS.contains(&n.to_lowercase().as_str())),
        Expr::Paren(x) => e_data(x),
        _ => false,
    }
}

/// `Range("A1:A9,C1:C9")`: a API das macros fala de uma área por vez, então a instrução
/// repete para cada uma (`$area`). `None` quando o endereço é de uma área só.
fn por_area(o: &Obj) -> Option<(Obj, String)> {
    let f = match o {
        Obj::Faixa(f) | Obj::Parte(f, _) => f,
        _ => return None,
    };
    // Só texto ou modelo de texto: numa chamada (`vba.fim(ws, …)`) a vírgula separa argumentos.
    let fora_de_chaves = f.end.starts_with(['"', '`']) && {
        let mut nivel = 0;
        f.end.chars().any(|c| match c {
            '{' => { nivel += 1; false }
            '}' => { nivel -= 1; false }
            ',' => nivel == 0,
            _ => false,
        })
    };
    if !fora_de_chaves {
        return None;
    }
    let um = Faixa { end: "$area".into(), literal: None, ..f.clone() };
    let end = f.end.clone();
    Some((match o {
        Obj::Parte(_, p) => Obj::Parte(um, p.clone()),
        _ => Obj::Faixa(um),
    }, end))
}
