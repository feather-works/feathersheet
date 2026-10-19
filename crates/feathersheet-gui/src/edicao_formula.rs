//! Rascunho de fórmula: autocomplete e apontamento sem gravar na planilha.
use feathersheet_core::{
    celulas::refs::Rect,
    formula::assistente::{accepts_reference, suggest_at, toggle_reference, token_before, Sugestao},
};

#[derive(Debug,Default)]
pub struct EditorFormula {
    pub ativo: bool,
    pub modo_apontar: bool,
    pub modo_texto: bool,
    pub origem: Option<(usize,u32,u32)>,
    pub cursor: Option<usize>,
    pub selecao_texto: Option<(usize,usize)>,
    pub ponto: Option<(u32,u32)>,
    pub ancora: Option<(u32,u32)>,
    trecho: Option<(usize,usize)>,
    pub sugestao: usize,
    ocultar_sugestoes: bool,
}
impl EditorFormula {
    pub fn encerrar(&mut self) { *self=Self::default(); }
    /// Localiza o ponto da alteração sem assumir que o cursor está no fim.
    pub fn alterado(&mut self, antes:&str, depois:&str) {
        let a:Vec<char>=antes.chars().collect();
        let b:Vec<char>=depois.chars().collect();
        let mut prefixo=0;
        while prefixo<a.len() && prefixo<b.len() && a[prefixo]==b[prefixo] {prefixo+=1;}
        let mut sufixo=0;
        while sufixo<a.len().saturating_sub(prefixo) &&
              sufixo<b.len().saturating_sub(prefixo) &&
              a[a.len()-1-sufixo]==b[b.len()-1-sufixo] {sufixo+=1;}
        self.digitou();
        self.cursor=Some(b.len().saturating_sub(sufixo));
    }
    pub fn posicao(&self, texto:&str)->usize {
        self.cursor.unwrap_or_else(||texto.chars().count()).min(texto.chars().count())
    }
    pub fn posicionar(&mut self, texto:&str, cursor:usize) {
        self.cursor=Some(cursor.min(texto.chars().count()));
        self.selecao_texto=None;
        self.trecho=None;
        self.ponto=None;
        self.ancora=None;
    }
    /// Seleção real do TextInput, em caracteres Unicode (não bytes).
    /// A seleção só pode virar referência quando a fórmula aceita uma
    /// referência nessa posição ou a seleção cobre uma referência existente.
    pub fn selecionar_texto(&mut self, texto:&str, inicio:usize, fim:usize) {
        let n=texto.chars().count();
        let (a,b)=(inicio.min(n),fim.min(n));
        self.posicionar(texto,b);
        if a<b {self.selecao_texto=Some((a,b));}
    }
    fn selecao_substituivel(&self,texto:&str)->bool {
        let Some((a,b))=self.selecao_texto else {return false;};
        a<b && (accepts_reference(texto,a)
            || feathersheet_core::formula::assistente::reference_boxes(texto)
                .iter().any(|r|r.start==a && r.end==b))
    }
    pub fn digitou(&mut self) {
        self.modo_apontar=false;
        self.modo_texto=false;
        self.ponto=None;
        self.ancora=None;
        self.trecho=None;
        self.sugestao=0;
        self.ocultar_sugestoes=false;
        self.cursor=None;
        self.selecao_texto=None;
    }
    pub fn sugestoes(&self, texto:&str, ingles:bool)->Vec<Sugestao> {
        if !self.ativo || self.ocultar_sugestoes { return Vec::new(); }
        if crate::idioma::lingua()==crate::idioma::Lingua::Es {
            return sugestoes_es(texto,self.posicao(texto),6);
        }
        suggest_at(texto,self.posicao(texto),6,ingles)
    }
    pub fn pode_apontar(&self, texto:&str)->bool {
        self.ativo && texto.starts_with('=') &&
        (self.trecho.is_some() || self.selecao_substituivel(texto)
            || accepts_reference(texto,self.posicao(texto)))
    }
    pub fn alternar_apontamento(&mut self) {
        self.modo_texto=!self.modo_texto;
        self.modo_apontar=!self.modo_texto;
    }
    pub fn referencia(&mut self, texto:&mut String, row:u32, col:u32, estender:bool)->bool {
        self.referencia_na_aba(texto,row,col,estender,None)
    }
    pub fn referencia_na_aba(&mut self, texto:&mut String, row:u32, col:u32,
        estender:bool, outra_aba:Option<&str>)->bool {
        let ancora=if estender {self.ancora.unwrap_or((row,col))} else {(row,col)};
        self.referencia_faixa(texto, ancora, (row,col), outra_aba)
    }

    /// Escreve a referência de `ancora` a `ponto` no rascunho. Colunas
    /// inteiras saem como `C:I` e linhas inteiras como `5:7`, como no Excel.
    pub fn referencia_faixa(&mut self, texto:&mut String, ancora:(u32,u32), ponto:(u32,u32),
        outra_aba:Option<&str>)->bool {
        if !self.pode_apontar(texto) && !self.modo_apontar {return false;}
        let (row,col)=ponto;
        let rect=Rect::new(ancora.0,ancora.1,row,col).normalized();
        let nome=crate::app::endereco_area(&rect);
        let nome=if let Some(aba)=outra_aba {
            format!("'{}'!{}",aba.replace("'","''"),nome)
        } else {nome};
        if !self.trocar_trecho(texto,&nome) {return false;}
        self.ancora=Some(ancora);
        self.ponto=Some((row,col));
        true
    }

    /// Referência já escrita (`[Vendas.fshz]Dados!A1`, clicada noutra
    /// janela): entra no lugar da apontada antes, como um clique na grade.
    pub fn referencia_pronta(&mut self, texto:&mut String, nome:&str)->bool {
        if !self.pode_apontar(texto) && !self.modo_apontar {return false;}
        if !self.trocar_trecho(texto,nome) {return false;}
        // A próxima seta na própria grade começa de novo.
        self.ancora=None;
        self.ponto=None;
        true
    }

    /// Põe `nome` no trecho apontado (ou na seleção, ou no cursor).
    fn trocar_trecho(&mut self, texto:&mut String, nome:&str)->bool {
        let n=texto.chars().count();
        let (ini,fim)=self.trecho
            .or(self.selecao_texto.filter(|_|self.selecao_substituivel(texto)))
            .unwrap_or((self.posicao(texto),self.posicao(texto)));
        if ini>n || fim>n || ini>fim {return false;}
        let a=byte_em_char(texto,ini);
        let b=byte_em_char(texto,fim);
        texto.replace_range(a..b,nome);
        self.trecho=Some((ini,ini+nome.chars().count()));
        self.cursor=Some(ini+nome.chars().count());
        self.selecao_texto=None;
        self.modo_apontar=true;
        self.modo_texto=false;
        self.ocultar_sugestoes=true;
        true
    }
    pub fn aceitar_sugestao(&mut self,texto:&mut String,indice:usize,ingles:bool)->bool {
        let sugestoes=self.sugestoes(texto,ingles);
        let Some(s)=sugestoes.get(indice) else {return false;};
        let nome=s.name.clone();
        let pos=self.posicao(texto);
        let Some(token)=token_before(texto,pos) else {return false;};
        let ini=byte_em_char(texto,token.start);
        let fim=byte_em_char(texto,pos);
        texto.replace_range(ini..fim,&format!("{nome}("));
        self.digitou();
        self.cursor=Some(token.start+nome.chars().count()+1);
        true
    }
    pub fn esconder(&mut self){self.ocultar_sugestoes=true;}
    pub fn alternar_referencia(&mut self, texto: &mut String) -> bool {
        if !self.ativo || !texto.starts_with('=') { return false; }
        let cursor = self.posicao(texto);
        let Some(caixa) = feathersheet_core::formula::assistente::reference_boxes(texto)
            .into_iter().find(|caixa| caixa.start <= cursor && cursor <= caixa.end)
        else { return false; };
        let Some(novo) = toggle_reference(texto, cursor) else { return false; };
        // Os cifrões adicionados por F4 mudam o comprimento da referência,
        // mas não devem jogar o caret para o fim de toda a fórmula.
        let novo_fim = caixa.end + novo.chars().count() - texto.chars().count();
        let novo_cursor = if cursor == caixa.end { novo_fim }
            else { cursor.min(novo_fim) };
        *texto = novo;
        self.digitou();
        self.cursor = Some(novo_cursor);
        true
    }
}
fn byte_em_char(s:&str,pos:usize)->usize {
    s.char_indices().nth(pos).map(|(i,_)|i).unwrap_or(s.len())
}
#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn autocomplete_portugues() {
        let mut e=EditorFormula::default();e.ativo=true;
        assert!(e.sugestoes("=SOM",false).iter().any(|s|s.name=="SOMA"));
        assert!(e.sugestoes("SOM",false).is_empty());
        assert!(e.sugestoes("=A1",false).is_empty());
        assert!(e.sugestoes("=SOMA(",false).is_empty());
        assert!(e.sugestoes("=SOM",true).iter().all(|s|s.name!="SOMA"), "em inglês, só nomes em inglês");
        assert!(e.sugestoes("=SU",true).iter().any(|s|s.name=="SUM"));
        let mut t="=SO".to_owned();
        let i=e.sugestoes(&t,false).iter().position(|s|s.name=="SOMA").unwrap();
        assert!(e.aceitar_sugestao(&mut t,i,false));
        assert_eq!(t,"=SOMA(");
    }
    #[test]
    fn mouse_e_shift_substituem_mesmo_trecho_de_referencia() {
        let mut e=EditorFormula::default();e.ativo=true;
        let mut t="=SOMA(".to_owned();
        assert!(e.referencia(&mut t,0,0,false));
        assert!(e.referencia(&mut t,2,3,true));
        assert_eq!(t,"=SOMA(A1:D3");
        e.digitou();
        t.push(';');
        assert!(e.referencia(&mut t,4,1,false));
        assert_eq!(t,"=SOMA(A1:D3;B5");
    }
    #[test]
    fn formula_no_meio_preserva_sufixo_com_utf8() {
        let mut e=EditorFormula::default();e.ativo=true;
        let mut texto="=SOMA(;MÉDIA(B2))".to_owned();
        e.posicionar(&texto,6);
        assert!(e.referencia(&mut texto,0,0,false));
        assert_eq!(texto,"=SOMA(A1;MÉDIA(B2))");
        assert_eq!(e.cursor,Some(8));
    }
    #[test]
    fn troca_de_aba_mantem_prefixo_e_aspas() {
        let mut e=EditorFormula::default();e.ativo=true;e.origem=Some((0,0,0));
        let mut texto="=SOMA(".to_owned();
        assert!(e.referencia_na_aba(&mut texto,2,3,false,Some("Base de Vendas")));
        assert_eq!(texto,"=SOMA('Base de Vendas'!D3");
        assert!(e.referencia_na_aba(&mut texto,4,4,true,Some("Base de Vendas")));
        assert_eq!(texto,"=SOMA('Base de Vendas'!D3:E5");
    }
    #[test]
    fn diff_localiza_cursor_depois_da_insercao() {
        let mut e=EditorFormula::default();e.ativo=true;
        e.alterado("=A1+B2","=A1+;B2");
        assert_eq!(e.cursor,Some(5));
    }
    #[test]
    fn nao_insere_ref_em_texto_e_preserva_utf8() {
        let mut e=EditorFormula::default();e.ativo=true;
        let mut t="texto".to_owned();
        assert!(!e.referencia(&mut t,0,0,false));
        t="=MÉDIA(".into();
        assert!(e.referencia(&mut t,0,0,false));
        assert!(e.referencia(&mut t,1,0,true));
        assert_eq!(t,"=MÉDIA(A1:A2");
    }

    #[test]
    fn selecao_textual_substitui_apenas_referencia_existente() {
        let mut e=EditorFormula::default(); e.ativo=true;
        let mut t="=A1+B2+C3".to_owned();
        e.selecionar_texto(&t,4,6);
        assert_eq!(e.selecao_texto,Some((4,6)));
        assert!(e.referencia(&mut t,5,3,false));
        assert_eq!(t,"=A1+D6+C3");
        assert_eq!(e.cursor,Some(6));
        assert_eq!(e.selecao_texto,None);
    }
    #[test]
    fn selecao_em_literal_de_texto_nao_substitui_por_referencia() {
        let mut e=EditorFormula::default();e.ativo=true;
        let mut t="=\"A1\"+B2".to_owned();
        e.selecionar_texto(&t,2,4);
        assert!(!e.pode_apontar(&t));
        assert!(!e.referencia(&mut t,4,4,false));
        assert_eq!(t,"=\"A1\"+B2");
    }
    #[test]
    fn f4_no_meio_nao_altera_outras_referencias_nem_sufixo() {
        let mut e=EditorFormula::default();e.ativo=true;
        let mut t="=SOMA(A1;B2)+C3".to_owned();
        e.posicionar(&t,8); // dentro de A1? índice 8: após A1
        assert!(e.alternar_referencia(&mut t));
        assert_eq!(t,"=SOMA($A$1;B2)+C3");
        assert_eq!(e.cursor,Some(10));
        e.posicionar(&t,10);
        assert!(e.alternar_referencia(&mut t));
        assert_eq!(t,"=SOMA(A$1;B2)+C3");
        e.posicionar(&t,9);
        assert!(e.alternar_referencia(&mut t));
        assert_eq!(t,"=SOMA($A1;B2)+C3");
    }
    #[test]
    fn f4_respeita_referencia_de_outra_aba_e_literais() {
        let mut e=EditorFormula::default();e.ativo=true;
        let mut t="=\"A1\"+'Base de Vendas'!B2+C3".to_owned();
        e.posicionar(&t,3);
        assert!(!e.alternar_referencia(&mut t));
        let pos=t.find("!B2").unwrap()+2;
        e.posicionar(&t,pos);
        assert!(e.alternar_referencia(&mut t));
        assert_eq!(t,"=\"A1\"+'Base de Vendas'!$B$2+C3");
        assert_eq!(e.cursor,Some(pos));
    }
    #[test]
    fn f4_alterna_referencia_no_fim_da_formula() {
        let mut e=EditorFormula::default(); e.ativo=true;
        let mut t="=A1+B2".to_owned();
        assert!(e.alternar_referencia(&mut t));
        assert_eq!(t,"=A1+$B$2");
    }
}

/// Autocompletar em espanhol: os nomes do de-para (`funcoes.es-ES.json`)
/// que começam com o que foi digitado; a ajuda vem do nome canônico.
fn sugestoes_es(texto:&str, cursor:usize, limite:usize)->Vec<Sugestao> {
    use feathersheet_core::formula::assistente::describe;
    if !texto.starts_with('=') { return Vec::new(); }
    // O motor diz se o cursor está num nome de função (mesma regra do pt/en):
    // com prefixo vazio de sugestões ele não serve; recorta o token aqui.
    let Some(token)=token_before(texto,cursor) else { return Vec::new(); };
    if token.start==0 { return Vec::new(); }
    let prefixo=token.text.to_uppercase();
    if prefixo.is_empty() || prefixo.chars().next().is_some_and(|c| !c.is_alphabetic()) { return Vec::new(); }
    if texto.chars().nth(token.end)==Some('(') { return Vec::new(); }
    let nomes=&crate::idioma::funcoes_de(crate::idioma::Lingua::Es).nomes;
    let mut achados:Vec<(&String,&String)>=nomes.iter()
        .filter(|(c,es)| es.starts_with(&prefixo)
            && feathersheet_core::funcoes::function_help::help_for(c).is_some())
        .collect();
    achados.sort_by(|a,b| a.1.chars().count().cmp(&b.1.chars().count()).then_with(|| a.1.cmp(b.1)));
    achados.into_iter().take(limite).map(|(c,es)| {
        let mut s=describe(c);
        s.name=es.clone();
        if let Some(h)=s.help { s.signature=crate::idioma::assinatura(c,h); }
        s
    }).collect()
}
