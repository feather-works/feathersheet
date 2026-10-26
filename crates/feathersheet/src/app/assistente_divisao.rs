//! Assistente de Texto para Colunas: quatro etapas, nenhuma barra lateral.
//! As escolhas ficam em rascunho até Concluir/Pular; o processamento é em lotes.
use super::*;
use std::borrow::Cow;
use feathersheet_core::{Value, operacoes::ops::{self, OpcoesDivisao, TipoColunaDivisao, FormatoDeData}};
mod visual;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo { Delimitado, LarguraFixa }

#[derive(Debug, Clone)]
pub enum Evento {
    IrPara(u8), Avancar, Voltar, Pular, Concluir, Cancelar, ProcessarLote,
    Modo(Modo), Separador(&'static str), Outro(String),
    Consecutivos(bool), Qualificador(&'static str), Largura(String),
    Coluna(usize), Tipo(&'static str), TipoNaColuna(usize, &'static str),
    Avancado, UsarCelulaAtual, NovaAba(bool),
    Destino(String), NomeAba(String),
    /// Confirmação do aviso "já contém dados": regravar por cima.
    Substituir,
}

pub(super) struct Estado {
    pub etapa:u8,
    pub origem:usize,
    pub area:Rect,
    pub modo:Modo,
    pub ponto_virgula:bool,
    pub virgula:bool,
    pub tab:bool,
    pub espaco:bool,
    pub outro_ativo:bool,
    pub outro:String,
    pub consecutivos:bool,
    pub qualificador:&'static str,
    pub largura:String,
    pub selecionada:usize,
    pub tipos:Vec<TipoColunaDivisao>,
    pub avancado:bool,
    pub nova_aba:bool,
    pub destino:String,
    pub nome_aba:String,
    pub erro:String,
    /// Primeira célula ocupada no destino; o aviso mostra o botão Substituir.
    pub conflito:Option<String>,
    /// Confirmado pela pessoa: a validação deixa de parar em célula com dado.
    pub substituir:bool,
    trabalho:Option<TrabalhoDivisao>,
}
/// Fases separadas para não bloquear a thread do Iced em seleções grandes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaseDivisao { Validando, Gravando }

/// O plano ocupa memória proporcional às fórmulas, nunca às células literais.
struct TrabalhoDivisao {
    fase:FaseDivisao,
    origem:usize,
    destino:Option<usize>,
    area:Rect,
    fim:u32,
    dst_row:u32,
    dst_col:u32,
    tipos:Vec<TipoColunaDivisao>,
    nova:bool,
    nome:String,
    modo:Modo,
    largura:usize,
    opcoes:OpcoesDivisao,
    formulas:std::collections::HashMap<u32,String>,
    cursor:u32,
    total:usize,
    cols:usize,
    com_divisao:bool,
    substituir:bool,
    iniciado_em:std::time::Instant,
    lote_gravar:u32,
    lote_validar:u32,
    rss_inicio_kb:u64,
    rss_pico_kb:u64,
    /// Gravação na mesma aba pelo caminho em lote (None: nova aba).
    lote:Option<LoteParalelo>,
    /// O que a barra mostra: persegue o percentual real a cada tique
    /// (12 ms), em vez de pular de lote em lote.
    exibido:f32,
}

type Preparadas=Vec<(u32,Vec<feathersheet_core::planilha::sheet::edicao::CampoDivisao>)>;

/// O corte e a análise rodam em threads; a janela só encaixa o resultado,
/// em fatias curtas, para continuar desenhando.
struct LoteParalelo {
    plano:std::sync::Arc<ops::PlanoDivisao>,
    gravador:feathersheet_core::planilha::sheet::edicao::GravadorDivisao,
    /// Próxima linha da origem a mandar para uma thread.
    leitura:u32,
    em_voo:std::collections::VecDeque<std::sync::mpsc::Receiver<Preparadas>>,
    prontas:std::collections::VecDeque<(u32,Vec<feathersheet_core::planilha::sheet::edicao::CampoDivisao>)>,
}
/// Linhas por pedido a uma thread.
const LINHAS_POR_PEDIDO:u32=2048;
/// Teto de linhas cortadas esperando a gravação. Cortar é muito mais rápido
/// que gravar: sem teto a fila crescia até 10 GB no CSV do Spotify (13/10).
const LINHAS_NA_FILA:usize=8192;
/// Quanto a gravação ocupa da thread da janela a cada tique (12 ms).
/// Idem para a validação.
const ORCAMENTO_VALIDAR:std::time::Duration=std::time::Duration::from_millis(6);
const ORCAMENTO_GRAVAR:std::time::Duration=std::time::Duration::from_millis(7);
/// Mira ~30 ms por tick: novo = atual × 30 ms / medido, crescendo no máximo
/// 2× por tick para não oscilar. Vale para validação e gravação: linhas
/// largas (83 colunas) travavam a janela com lotes fixos.
fn ajustar_lote(lote:&mut u32, decorrido:std::time::Duration, piso:u32, teto:u32) {
    let ms=decorrido.as_secs_f64()*1000.0;
    let alvo=*lote as f64*30.0/ms.max(1.0);
    let subida=(*lote as f64*2.0).min(teto as f64);
    *lote=alvo.min(subida).clamp(piso as f64,teto as f64) as u32;
}
impl TrabalhoDivisao {
    fn percentual(&self) -> u32 {
        let linhas = (self.fim as u64 - self.area.r0 as u64 + 1).max(1);
        let feitas = self.cursor.saturating_sub(self.area.r0) as u64;
        let parcial = (feitas.min(linhas)*50/linhas) as u32;
        if self.fase==FaseDivisao::Validando {parcial} else {50+parcial}
    }
    fn progresso(&self) -> String {
        let fase = if self.fase==FaseDivisao::Validando {tr!("Validando destinos")}
            else {tr!("Convertendo linhas")};
        trf!("{fase} · {}% · {} de {} linhas · RSS +{:.0} MiB (pico {:.0} MiB)",
            self.percentual(),self.cursor.saturating_sub(self.area.r0),
            self.fim-self.area.r0+1,
            self.rss_pico_kb.saturating_sub(self.rss_inicio_kb) as f64 / 1024.0,
            self.rss_pico_kb as f64 / 1024.0)
    }
}
// Validação e gravação adaptam o tamanho do lote pela latência observada.
const LOTE_VALIDAR_INICIAL: u32 = 1024;
const LOTE_VALIDAR_MIN: u32 = 64;
const LOTE_VALIDAR_MAX: u32 = 8_192;
const LOTE_GRAVAR: u32 = 128;
const LOTE_GRAVAR_MIN: u32 = 16;
const LOTE_GRAVAR_MAX: u32 = 2_048;
impl Estado {
    fn novo(app:&App)->Self {
        let area=app.area_selecionada();
        let destino=format!("=${}${}",feathersheet_core::col_name(area.c0),area.r0+1);
        Self {
            etapa:1, origem:app.folha, area,
            modo:Modo::Delimitado, ponto_virgula:true,virgula:false,
            tab:false,espaco:false,outro_ativo:false,outro:String::new(),
            consecutivos:true, qualificador:tr!("Aspas duplas"),
            largura:"10".into(), selecionada:0, tipos:Vec::new(), avancado:false,
            nova_aba:false, destino,
            nome_aba:"Dados_Separados".into(), erro:String::new(),
            conflito:None, substituir:false,
            trabalho:None,
        }
    }
    pub(super) fn conversao_em_andamento(&self)->bool {
        self.trabalho.is_some()
    }
    pub(super) fn percentual_exibido(&self)->f32 {
        self.trabalho.as_ref().map_or(0.0,|t|t.exibido)
    }
    pub(super) fn progresso_conversao(&self)->Option<String> {
        self.trabalho.as_ref().map(TrabalhoDivisao::progresso)
    }
    fn escolher_separador(&mut self,chave:&str) {
        self.ponto_virgula=chave==";";
        self.virgula=chave==",";
        self.tab=chave=="Tab";
        self.espaco=chave=="Espaço";
        self.outro_ativo=chave=="Outro";
    }
    /// Chave do separador marcado, para o rádio da tela de delimitadores.
    pub(super) fn separador_atual(&self)->&'static str {
        if self.virgula {","} else if self.tab {"Tab"} else if self.espaco {"Espaço"}
        else if self.outro_ativo {"Outro"} else {";"}
    }
    fn opcoes(&self)->OpcoesDivisao {
        let mut delims=Vec::new();
        if self.ponto_virgula {delims.push(";".into());}
        if self.virgula {delims.push(",".into());}
        if self.tab {delims.push("\t".into());}
        if self.espaco {delims.push(" ".into());}
        if self.outro_ativo&&!self.outro.is_empty() {delims.push(self.outro.clone());}
        OpcoesDivisao {
            delimitadores:delims,consecutivos_como_um:self.consecutivos,
            qualificador_texto:match self.qualificador {
                "Aspas duplas"=>Some('"'),
                "Aspas simples"=>Some('\''),
                _=>None,
            },
        }
    }
    fn partes(&self, texto:&str)->Vec<String> {
        // Prévia de uma única linha; o processamento em lotes prepara
        // opções uma única vez e as reutiliza durante toda a operação.
        partes_preparadas(
            texto, self.modo,
            self.largura.parse::<usize>().unwrap_or(10).clamp(1,16384),
            &self.opcoes(),
        )
    }
    fn tipo(&self,i:usize)->TipoColunaDivisao {
        self.tipos.get(i).cloned().unwrap_or(TipoColunaDivisao::Geral)
    }
    fn validacao(&self)->Result<(),String> {
        if self.area.c0!=self.area.c1 {return Err(tr!("Selecione apenas uma coluna na planilha.").into());}
        if self.modo==Modo::LarguraFixa && self.largura.parse::<usize>().ok().filter(|n| (1..=16384).contains(n)).is_none() {
            return Err(tr!("A largura fixa deve ter entre 1 e 16.384 caracteres.").into());
        }
        if self.modo==Modo::Delimitado && self.opcoes().delimitadores.is_empty() {
            return Err(tr!("Selecione pelo menos um separador.").into());
        }
        if self.nova_aba&&self.nome_aba.trim().is_empty() {return Err(tr!("Informe o nome da nova planilha.").into());}
        if !self.nova_aba&&endereco(&self.destino).is_none() {return Err(tr!("Informe um destino válido, como A1.").into());}
        Ok(())
    }
}
/// Separa largura fixa por offsets UTF-8, sem criar Vec<char> por célula.
/// Em modo delimitado reutiliza as opções preparadas uma vez no assistente.
/// A validação apenas conta; somente a gravação cria os campos. Na
/// seleção de 1M de linhas, isso evita milhões de Strings transitórios.
/// Mesma regra de `get_raw(..).is_empty()`, sem alocar String para texto e
/// célula literal: `texto_cru_divisao` empresta do pool. Só números e
/// fórmulas (que não têm texto emprestável) caem no `get_raw`.
fn celula_com_conteudo(sheet:&feathersheet_core::Sheet,linha:u32,col:u32)->bool {
    match sheet.texto_cru_divisao(linha,col) {
        Some(s)=>!s.is_empty(),
        None=>!sheet.get_raw(linha,col).is_empty(),
    }
}

fn contar_partes_preparadas(texto: &str, modo: Modo, largura: usize,
    opcoes: &OpcoesDivisao) -> usize {
    if modo != Modo::LarguraFixa {
        return ops::contar_colunas_divisao(texto, opcoes);
    }
    texto.chars().count().div_ceil(largura.max(1))
}

fn partes_preparadas(
    texto: &str, modo: Modo, largura: usize, opcoes: &OpcoesDivisao
) -> Vec<String> {
    if modo != Modo::LarguraFixa {
        return ops::dividir_com_opcoes(texto, opcoes);
    }
    let largura = largura.max(1);
    let mut partes = Vec::new();
    let mut inicio = 0;
    let mut tamanho = 0;
    for (indice, _) in texto.char_indices() {
        if tamanho == largura {
            partes.push(texto[inicio..indice].to_owned());
            inicio = indice;
            tamanho = 0;
        }
        tamanho += 1;
    }
    if inicio < texto.len() {
        partes.push(texto[inicio..].to_owned());
    }
    partes
}

/// Compartilhado pelas ações do campo de formato e por coluna.
fn tipo_de_rotulo(tipo:&str)->TipoColunaDivisao {
    // A lista mostra o rótulo traduzido; a decisão usa a chave em português.
    let tipo=crate::idioma::chave_de(&[tr!("Número"),tr!("Texto"),tr!("Porcentagem"),tr!("Data DMA"),tr!("Data MDA"),tr!("Data AMD"),tr!("Ignorar")],tipo)
        .unwrap_or(tipo);
    match tipo {
        "Número"=>TipoColunaDivisao::Numero,
        "Texto"=>TipoColunaDivisao::Texto,
        "Porcentagem"=>TipoColunaDivisao::Porcentagem,
        "Data DMA"=>TipoColunaDivisao::Data(FormatoDeData::DiaMesAno),
        "Data MDA"=>TipoColunaDivisao::Data(FormatoDeData::MesDiaAno),
        "Data AMD"=>TipoColunaDivisao::Data(FormatoDeData::AnoMesDia),
        "Ignorar"=>TipoColunaDivisao::Pular,
        _=>TipoColunaDivisao::Geral,
    }
}

fn texto_valor(v:Value)->String {
    match v {
        Value::Empty=>String::new(),Value::Text(s)=>s,
        Value::Number(n)=>format!("{n}"),Value::Bool(b)=>b.to_string(),
        Value::Error(e)=>e.to_string(),
    }
}
/// A1, $A$1 e referencias absolutas são aceitas. Não aceitamos intervalo ou aba.
fn endereco(s:&str)->Option<(u32,u32)> {
    let limpo=s.trim().trim_start_matches('=').replace('$',"");
    let i=limpo.find(|ch:char|ch.is_ascii_digit())?;
    let (letras,linhas)=limpo.split_at(i);
    if letras.is_empty() || !letras.chars().all(|x|x.is_ascii_alphabetic()) ||
        linhas.is_empty() || !linhas.chars().all(|x|x.is_ascii_digit()) {
        return None;
    }
    let c=col_index(&letras.to_ascii_uppercase())?;
    let r=linhas.parse::<u32>().ok()?.checked_sub(1)?;
    (r<MAX_ROWS&&c<MAX_COLS).then_some((r,c))
}

impl App {
    pub(super) fn abrir_assistente_divisao(&mut self)->Task<Msg> {
        self.confirmar();
        if self.janelas.aberta(Auxiliar::DividirTexto) {return Task::none();}
        self.assistente_divisao=Some(Estado::novo(self));
        self.alternar(Auxiliar::DividirTexto)
    }
    pub(super) fn evento_assistente_divisao(&mut self,evento:Evento)->Task<Msg> {
        if matches!(evento,Evento::ProcessarLote) {
            return self.processar_lote_divisao();
        }
        if matches!(evento,Evento::Cancelar) {
            self.cancelar_trabalho_divisao();
            self.assistente_divisao=None;
            return self.janelas.fechar_tipo(Auxiliar::DividirTexto);
        }
        if matches!(evento,Evento::Substituir) {
            if let Some(e)=self.assistente_divisao.as_mut() {
                e.substituir=true;
                e.conflito=None;
            }
            if let Err(erro)=self.iniciar_trabalho_divisao() {
                if let Some(e)=self.assistente_divisao.as_mut() {e.erro=erro;}
            }
            return Task::none();
        }
        if matches!(evento,Evento::Concluir|Evento::Pular) {
            if let Err(erro)=self.iniciar_trabalho_divisao() {
                if let Some(e)=self.assistente_divisao.as_mut() {e.erro=erro;}
            }
            return Task::none();
        }
        // Enquanto valida/grava, o rascunho permanece congelado.
        if self.assistente_divisao.as_ref().is_some_and(|e| e.trabalho.is_some()) {
            return Task::none();
        }
        let Some(e)=self.assistente_divisao.as_mut() else {return Task::none();};
        e.erro.clear();
        // Qualquer mudança no assistente pede nova confirmação.
        e.conflito=None;
        e.substituir=false;
        match evento {
            Evento::IrPara(n)=>e.etapa=n.clamp(1,4),
            Evento::Avancar=>e.etapa=(e.etapa+1).min(4),
            Evento::Voltar=>e.etapa=e.etapa.saturating_sub(1).max(1),
            Evento::Modo(m)=>e.modo=m,
            Evento::Separador(chave)=>match chave {
                // Um separador só por vez: escolher um desmarca os outros.
                ";"|","|"Tab"|"Espaço"|"Outro"=>e.escolher_separador(chave),
                _=>{}
            },
            Evento::Outro(v)=>{e.outro=v;e.escolher_separador("Outro");},
            Evento::Consecutivos(v)=>e.consecutivos=v,
            Evento::Qualificador(v)=>e.qualificador=v,
            Evento::Largura(v)=>e.largura=v,
            Evento::Coluna(i)=>e.selecionada=i,
            Evento::Tipo(tipo)=>{
                let i=e.selecionada;
                if e.tipos.len()<=i {e.tipos.resize(i+1,TipoColunaDivisao::Geral);}
                e.tipos[i]=tipo_de_rotulo(tipo);
            },
            Evento::TipoNaColuna(i,tipo)=>{
                e.selecionada=i;
                if e.tipos.len()<=i {e.tipos.resize(i+1,TipoColunaDivisao::Geral);}
                e.tipos[i]=tipo_de_rotulo(tipo);
            },
            Evento::Avancado=>e.avancado=!e.avancado,
            Evento::UsarCelulaAtual=>e.destino=cell_name(self.linha,self.coluna),
            Evento::NovaAba(b)=>e.nova_aba=b,
            Evento::Destino(s)=>e.destino=s,
            Evento::NomeAba(s)=>e.nome_aba=s,
            Evento::Concluir|Evento::Pular|Evento::Cancelar|Evento::ProcessarLote
                |Evento::Substituir=>unreachable!(),
        }
        Task::none()
    }
    /// Começa sem percorrer nenhuma linha: a validação roda em ticks de UI.
    fn iniciar_trabalho_divisao(&mut self)->Result<(),String> {
        let e=self.assistente_divisao.as_ref().ok_or(tr!("Assistente fechado"))?;
        if e.trabalho.is_some() { return Ok(()); }
        e.validacao()?;
        let aba=self.workbook.aba(e.origem).ok_or(tr!("Aba de origem não encontrada"))?;
        let fim=e.area.r1.min(aba.sheet.used_rect().r1);
        if e.area.r0>fim {return Err(tr!("Nenhuma linha preenchida na seleção").into());}
        let (dst_row,dst_col)=if e.nova_aba {(0,0)}else{endereco(&e.destino).unwrap()};
        // Verificar os limites de destino para o ÚLTIMO registro antes
        // de começar a escrita. Mesmo com o milhão de linhas isso é O(1).
        let ultima=dst_row as u64+fim as u64-e.area.r0 as u64;
        if ultima>=MAX_ROWS as u64 {
            return Err(tr!("A divisão ultrapassa a última linha da planilha").into());
        }
        let destino_txt=if e.nova_aba {trf!("nova aba '{}'",e.nome_aba.trim())}
            else {trf!("{} (própria aba)",e.destino)};
        let linha=trf!("início · área {}:{} · destino {} · modo {:?}",
            cell_name(e.area.r0,e.area.c0),cell_name(fim,e.area.c1),destino_txt,e.modo);
        self.monitor.registrar(feather_ui::monitor::Nivel::Info,tr!("Divisão"),&linha);
        let rss_inicio=Self::rss_atual_kb().unwrap_or(0);
        let trabalho=TrabalhoDivisao {
            fase:FaseDivisao::Validando,
            origem:e.origem,destino:None,area:e.area.clone(),fim,
            dst_row,dst_col,tipos:e.tipos.clone(),nova:e.nova_aba,
            nome:e.nome_aba.trim().to_string(),
            modo:e.modo,largura:e.largura.parse::<usize>().unwrap_or(10).clamp(1,16384),
            opcoes:e.opcoes(),
            formulas:std::collections::HashMap::new(),
            cursor:e.area.r0,total:0,cols:0,com_divisao:false,
            substituir:e.substituir,
            iniciado_em:std::time::Instant::now(),
            lote_gravar:LOTE_GRAVAR,lote_validar:LOTE_VALIDAR_INICIAL,
            rss_inicio_kb:rss_inicio,rss_pico_kb:rss_inicio,
            lote:None, exibido:0.0,
        };
        if let Some(estado)=self.assistente_divisao.as_mut() {
            estado.trabalho=Some(trabalho);
            estado.erro.clear();
            estado.conflito=None;
        }
        self.status=tr!("Conversão de texto em colunas iniciada").into();
        Ok(())
    }

    /// Interrompe sem deixar metade da operação escrita. O Ctrl+Z continua
    /// sendo uma só transação mesmo com centenas de lotes de escrita.
    pub(super) fn cancelar_trabalho_divisao(&mut self) {
        let Some(trabalho)=self.assistente_divisao.as_mut()
            .and_then(|e|e.trabalho.take()) else {return};
        if trabalho.fase==FaseDivisao::Gravando {
            if let Some(destino)=trabalho.destino {
                if trabalho.nova {
                    // Aba criada exclusivamente para esta conversão.
                    self.workbook.remover_aba(destino);
                } else {
                    self.workbook.editar(destino,|sheet|sheet.reverter_transacao());
                }
            }
        }
        self.status=tr!("Conversão cancelada; dados originais preservados").into();
    }

    /// Um tique da gravação em lote: manda linhas para as threads, recolhe as
    /// prontas na ordem e grava por no máximo [`ORCAMENTO_GRAVAR`].
    fn tique_lote_paralelo(&mut self,trabalho:&mut TrabalhoDivisao,destino:usize)->Result<(),String> {
        let lote=trabalho.lote.as_mut().expect("lote paralelo");
        let threads=feathersheet_core::paralelismo::nucleos();
        if let Some(aba)=self.workbook.aba(destino) {
            while lote.em_voo.len()<threads && lote.leitura<=lote.plano.r1
                && lote.prontas.len()+lote.em_voo.len()*(LINHAS_POR_PEDIDO as usize) < LINHAS_NA_FILA {
                let ate=lote.leitura.saturating_add(LINHAS_POR_PEDIDO-1).min(lote.plano.r1);
                let textos:Vec<(u32,String)>=(lote.leitura..=ate)
                    .map(|r|(r,lote.plano.texto_origem(&aba.sheet,r))).collect();
                let plano=std::sync::Arc::clone(&lote.plano);
                let (envio,recebe)=std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let prontas:Preparadas=textos.into_iter().filter(|(_,t)|!t.is_empty())
                        .map(|(r,t)|(plano.dst_row+(r-plano.r0),plano.preparar(&t))).collect();
                    let _=envio.send(prontas);
                });
                lote.em_voo.push_back(recebe);
                lote.leitura=ate.saturating_add(1);
                if ate==u32::MAX {break;}
            }
        }
        while let Some(recebe)=lote.em_voo.front() {
            match recebe.try_recv() {
                Ok(prontas)=>{lote.prontas.extend(prontas);lote.em_voo.pop_front();}
                Err(std::sync::mpsc::TryRecvError::Empty)=>break,
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>
                    return Err(tr!("Uma thread da conversão falhou; nada foi alterado.").into()),
            }
        }
        let inicio=std::time::Instant::now();
        let dst_col=lote.plano.dst_col;
        let mut ultima=None;
        self.workbook.editar_lote_sem_notificar(destino,|sheet| {
            while !lote.prontas.is_empty() && inicio.elapsed()<ORCAMENTO_GRAVAR {
                let n=lote.prontas.len().min(128);
                let fatia:Vec<_>=lote.prontas.drain(..n).collect();
                ultima=fatia.last().map(|(r,_)|*r);
                sheet.gravar_divisao(&mut lote.gravador,dst_col,&fatia);
            }
        });
        let terminou=lote.leitura>lote.plano.r1 && lote.em_voo.is_empty() && lote.prontas.is_empty();
        trabalho.cursor=if terminou {trabalho.fim.saturating_add(1)}
            else if let Some(r)=ultima {lote.plano.r0+(r-lote.plano.dst_row)+1}
            else {trabalho.cursor};
        Ok(())
    }

    /// Um único lote por chamada. O timer do Iced redistribui o controle
    /// entre etapas, permitindo redesenhar a janela e processar eventos.
    fn processar_lote_divisao(&mut self)->Task<Msg> {
        let Some(mut trabalho)=self.assistente_divisao.as_mut()
            .and_then(|e|e.trabalho.take()) else {return Task::none()};
        let inicio=trabalho.cursor;
        let inicio_lote=std::time::Instant::now();
        let estava_gravando=trabalho.fase==FaseDivisao::Gravando;
        if inicio>trabalho.fim {
            if trabalho.fase==FaseDivisao::Validando {
                if !trabalho.com_divisao {
                    if let Some(e)=self.assistente_divisao.as_mut() {
                        e.erro=tr!("Nenhum dado produziu mais de uma coluna. Confira os separadores.").into();
                    }
                    return Task::none();
                }
                let destino=if trabalho.nova {
                    let existentes=self.workbook.nomes();
                    let mut nome=trabalho.nome.clone();
                    if existentes.contains(&nome) {
                        let mut i=2;
                        while existentes.contains(&format!("{}_{i}",trabalho.nome)) {i+=1;}
                        nome=format!("{}_{i}",trabalho.nome);
                    }
                    self.workbook.criar_aba(nome)
                } else {trabalho.origem};
                let linha=trf!("validação concluída · {} linhas · até {} coluna(s) · {:.1} ms desde o início · lote validar {}",
                    trabalho.total,trabalho.cols,trabalho.iniciado_em.elapsed().as_secs_f64()*1000.0,
                    trabalho.lote_validar);
                self.monitor.registrar(feather_ui::monitor::Nivel::Info,tr!("Divisão"),&linha);
                trabalho.destino=Some(destino);
                trabalho.fase=FaseDivisao::Gravando;
                trabalho.cursor=trabalho.area.r0;
                self.workbook.editar_lote_sem_notificar(destino,|sheet|sheet.abrir_transacao());
                if !trabalho.nova {
                    // Mesma aba: desfazer leve + corte em paralelo.
                    let plano=std::sync::Arc::new(ops::PlanoDivisao {
                        col_origem:trabalho.area.c0, r0:trabalho.area.r0, r1:trabalho.fim,
                        dst_row:trabalho.dst_row, dst_col:trabalho.dst_col,
                        colunas:trabalho.cols as u32,
                        largura:(trabalho.modo==Modo::LarguraFixa).then_some(trabalho.largura),
                        opcoes:trabalho.opcoes.clone(), tipos:trabalho.tipos.clone(),
                        formulas:std::mem::take(&mut trabalho.formulas),
                    });
                    let t=std::time::Instant::now();
                    let mut gravador=None;
                    self.workbook.editar_lote_sem_notificar(destino,|sheet|
                        gravador=Some(ops::iniciar_divisao(sheet,std::sync::Arc::clone(&plano))));
                    let linha=trf!("foto do desfazer · {:.2} s",t.elapsed().as_secs_f64());
                    self.monitor.registrar(feather_ui::monitor::Nivel::Info,tr!("Divisão"),&linha);
                    trabalho.lote=gravador.map(|gravador| LoteParalelo {
                        leitura:plano.r0, plano, gravador,
                        em_voo:Default::default(), prontas:Default::default(),
                    });
                }
                self.status=tr!("Destinos validados; convertendo texto em colunas").into();
            } else {
                let mut fechar_ms=0.0;
                if let Some(destino)=trabalho.destino {
                    let t=std::time::Instant::now();
                    self.workbook.editar(destino,|sheet|sheet.fechar_transacao());
                    fechar_ms=t.elapsed().as_secs_f64()*1000.0;
                    if trabalho.nova {self.folha=destino;}
                }
                let linha=trf!("fim · {} linhas · até {} coluna(s) · fechar_transacao {:.1} ms · total {:.1} ms · pico RSS {:.0} MiB",
                    trabalho.total,trabalho.cols,fechar_ms,
                    trabalho.iniciado_em.elapsed().as_secs_f64()*1000.0,
                    trabalho.rss_pico_kb as f64 / 1024.0);
                self.monitor.registrar(feather_ui::monitor::Nivel::Info,tr!("Divisão"),&linha);
                self.alterado=true;
                self.status=trf!("{} linha(s) convertida(s) em até {} coluna(s) · {:.1}s · RSS início {:.0} MiB · pico {:.0} MiB",
                    trabalho.total,trabalho.cols,trabalho.iniciado_em.elapsed().as_secs_f32(),
                    trabalho.rss_inicio_kb as f64 / 1024.0,
                    trabalho.rss_pico_kb as f64 / 1024.0);
                self.assistente_divisao=None;
                App::devolver_memoria(tr!("texto para colunas"));
                return self.janelas.fechar_tipo(Auxiliar::DividirTexto);
            }
        } else {
            let quantidade=if trabalho.fase==FaseDivisao::Validando {
                trabalho.lote_validar
            }else{trabalho.lote_gravar};
            let mut fim_lote=inicio.saturating_add(quantidade-1).min(trabalho.fim);
            if trabalho.fase==FaseDivisao::Validando {
                let origem=trabalho.origem;
                let Some(aba)=self.workbook.aba(origem) else {return Task::none()};
                let mesma_coluna=trabalho.dst_col==trabalho.area.c0
                    && trabalho.dst_row==trabalho.area.r0;
                for r in inicio..=fim_lote {
                    // Orçamento de quadro: nunca passa de ~6 ms por tique.
                    if r>inicio && (r-inicio)%64==0 && inicio_lote.elapsed()>ORCAMENTO_VALIDAR {
                        fim_lote=r-1;
                        break;
                    }
                    // Para strings da fonte, a leitura empresta o &str do
                    // StringPool: nenhuma nova cópia de 100+ caracteres.
                    let raw: Cow<'_, str> =
                        match aba.sheet.texto_cru_divisao(r,trabalho.area.c0) {
                            Some(v) => Cow::Borrowed(v),
                            None => Cow::Owned(aba.sheet.get_raw(r,trabalho.area.c0)),
                        };
                    if raw.is_empty() {continue;}
                    let formula = raw.starts_with('=');
                    // No padrão do assistente a coluna é regravada no
                    // lugar; texto sem token não precisa ser convertido.
                    if trabalho.modo==Modo::Delimitado
                        && trabalho.opcoes.delimitadores.len()==1
                        && !formula
                        // Texto qualificado deve passar pelo parser mesmo
                        // sem token, para remover corretamente as aspas.
                        && !trabalho.opcoes.qualificador_texto
                            .is_some_and(|q| raw.contains(q))
                        && !raw.contains(&trabalho.opcoes.delimitadores[0])
                        && mesma_coluna {continue;}
                    let texto: Cow<'_,str> = if formula {
                        Cow::Owned(texto_valor(self.workbook.valor(origem,r,trabalho.area.c0)))
                    } else {Cow::Borrowed(raw.as_ref())};
                    if texto.is_empty(){continue;}
                    let quantidade=contar_partes_preparadas(texto.as_ref(),trabalho.modo,
                        trabalho.largura,&trabalho.opcoes);
                    if quantidade>1 {trabalho.com_divisao=true;}
                    let linha=trabalho.dst_row+(r-trabalho.area.r0);
                    if linha>=MAX_ROWS {
                        if let Some(e)=self.assistente_divisao.as_mut() {
                            e.erro=tr!("A divisão ultrapassa a última linha da planilha").into();
                        }
                        return Task::none();
                    }
                    for i in 0..quantidade {
                        if matches!(trabalho.tipos.get(i),Some(TipoColunaDivisao::Pular)) {
                            continue;
                        }
                        let col=trabalho.dst_col as usize+i;
                        if col>=MAX_COLS as usize {
                            if let Some(e)=self.assistente_divisao.as_mut() {
                                e.erro=tr!("A divisão ultrapassa XFD").into();
                            }
                            return Task::none();
                        }
                        let mesma=linha==r&&col==trabalho.area.c0 as usize;
                        if !trabalho.nova && !trabalho.substituir && !mesma
                            && celula_com_conteudo(&aba.sheet,linha,col as u32) {
                            if let Some(e)=self.assistente_divisao.as_mut() {
                                let celula=cell_name(linha,col as u32);
                                e.erro=trf!("{celula} já contém dados. Substituir os dados existentes no destino?");
                                e.conflito=Some(celula);
                            }
                            return Task::none();
                        }
                    }
                    if formula {
                        trabalho.formulas.insert(r,texto.into_owned());
                    }
                    trabalho.total+=1;
                    trabalho.cols=trabalho.cols.max(quantidade);
                }
            } else if trabalho.lote.is_some() {
                let destino=trabalho.destino.expect("destino criado após validação");
                if let Err(erro)=self.tique_lote_paralelo(&mut trabalho,destino) {
                    if let Some(e)=self.assistente_divisao.as_mut() {e.erro=erro;}
                    self.workbook.editar(destino,|sheet|sheet.reverter_transacao());
                    return Task::none();
                }
            } else {
                let destino=trabalho.destino.expect("destino criado após validação");
                let mut linhas=Vec::new();
                if trabalho.nova {
                    let Some(fonte)=self.workbook.aba(trabalho.origem) else {
                        return Task::none();
                    };
                    for r in inicio..=fim_lote {
                        let texto: Cow<'_,str> =
                            match fonte.sheet.texto_cru_divisao(r,trabalho.area.c0) {
                                Some(raw) if !raw.starts_with('=') => Cow::Borrowed(raw),
                                _ => Cow::Owned(texto_valor(
                                    self.workbook.valor(trabalho.origem,r,trabalho.area.c0))),
                            };
                        if !texto.is_empty() {
                            linhas.push((trabalho.dst_row+(r-trabalho.area.r0),
                                partes_preparadas(texto.as_ref(),trabalho.modo,
                                    trabalho.largura,&trabalho.opcoes)));
                        }
                    }
                } else {
                    // Ler e escrever na mesma aba na única transação.
                    self.workbook.editar_lote_sem_notificar(destino,|sheet| {
                        for r in inicio..=fim_lote {
                            let texto: Cow<'_,str> = if let Some(formula)=trabalho.formulas.get(&r) {
                                Cow::Borrowed(formula)
                            } else {
                                match sheet.texto_cru_divisao(r,trabalho.area.c0) {
                                    Some(raw) if !raw.starts_with('=') => Cow::Borrowed(raw),
                                    _ => Cow::Owned(texto_valor(sheet.get_value(r,trabalho.area.c0))),
                                }
                            };
                            if !texto.is_empty() {
                                linhas.push((trabalho.dst_row+(r-trabalho.area.r0),
                                    partes_preparadas(texto.as_ref(),trabalho.modo,
                                        trabalho.largura,&trabalho.opcoes)));
                            }
                        }
                        if !linhas.is_empty() {
                            ops::aplicar_divisao(sheet,trabalho.dst_col,&linhas,Some(&trabalho.tipos));
                            sheet.compactar_lote_divisao();
                        }
                    });
                }
                if trabalho.nova && !linhas.is_empty() {
                    self.workbook.editar_lote_sem_notificar(destino,|sheet| {
                        ops::aplicar_divisao(sheet,trabalho.dst_col,&linhas,Some(&trabalho.tipos));
                        sheet.compactar_lote_divisao();
                    });
                }
            }
            let decorrido=inicio_lote.elapsed();
            // Por lote nada vai ao console: só início, validação e fim (resumo).
            // O lote paralelo avança o cursor sozinho, só até a última linha
            // gravada: pular para `fim_lote` encerrava a conversão com as
            // threads ainda trabalhando, e as células ficavam sem o resultado.
            if !(estava_gravando && trabalho.lote.is_some()) {
                trabalho.cursor=fim_lote+1;
            }
            if estava_gravando {
                ajustar_lote(&mut trabalho.lote_gravar,decorrido,LOTE_GRAVAR_MIN,LOTE_GRAVAR_MAX);
            } else {
                ajustar_lote(&mut trabalho.lote_validar,decorrido,LOTE_VALIDAR_MIN,LOTE_VALIDAR_MAX);
            }
        }
        if let Some(rss)=Self::rss_atual_kb() {
            trabalho.rss_pico_kb=trabalho.rss_pico_kb.max(rss);
        }
        // Desliza um quinto da distância por tique: ~0,1 s para alcançar.
        let alvo=trabalho.percentual() as f32;
        trabalho.exibido+=(alvo-trabalho.exibido)*0.2;
        if let Some(e)=self.assistente_divisao.as_mut() {
            e.trabalho=Some(trabalho);
        }
        Task::none()
    }

}

#[cfg(test)]
mod testes {
    use super::*;
    fn finalizar_lotes(app:&mut App) {
        // As threads do lote paralelo levam alguns microssegundos: o tique
        // seguinte só recolhe o que já ficou pronto, como o timer do app.
        for _ in 0..2_000 {
            if !app.assistente_divisao.as_ref().is_some_and(Estado::conversao_em_andamento) {
                break;
            }
            app.evento_assistente_divisao(Evento::ProcessarLote);
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    }

    #[test]
    fn lote_de_escrita_adapta_tamanho_sem_perder_progresso() {
        let app=App::default();
        let e=Estado::novo(&app);
        let mut t=TrabalhoDivisao {
            fase:FaseDivisao::Gravando, origem:0, destino:Some(0),
            area:e.area.clone(), fim:0, dst_row:0, dst_col:0,
            tipos:Vec::new(), nova:false, nome:String::new(),
            modo:Modo::Delimitado, largura:10, opcoes:e.opcoes(),
            formulas:std::collections::HashMap::new(),
            cursor:0, total:0, cols:0, com_divisao:true, substituir:false,
            iniciado_em:std::time::Instant::now(), lote_gravar:LOTE_GRAVAR,
            lote_validar:LOTE_VALIDAR_INICIAL,
            rss_inicio_kb:0,rss_pico_kb:0, lote:None, exibido:0.0,
        };
        let ms=std::time::Duration::from_millis;
        // 128 × 30 / 140 = 27,4 → 27 (abaixo do dobro, sem corte)
        ajustar_lote(&mut t.lote_gravar,ms(140),LOTE_GRAVAR_MIN,LOTE_GRAVAR_MAX);
        assert_eq!(t.lote_gravar,27);
        // 27 × 30 / 200 = 4,05 → piso 16
        ajustar_lote(&mut t.lote_gravar,ms(200),LOTE_GRAVAR_MIN,LOTE_GRAVAR_MAX);
        assert_eq!(t.lote_gravar,16);
        // 16 × 30 / 1 = 480, mas o teto é 2× por tick: 32, 64, …, 2048 (7 ticks)
        for _ in 0..10 {
            ajustar_lote(&mut t.lote_gravar,ms(1),LOTE_GRAVAR_MIN,LOTE_GRAVAR_MAX);
        }
        assert_eq!(t.lote_gravar,2_048);
        // 2048 × 30 / 60 = 1024 (queda direta, sem limite)
        ajustar_lote(&mut t.lote_gravar,ms(60),LOTE_GRAVAR_MIN,LOTE_GRAVAR_MAX);
        assert_eq!(t.lote_gravar,1_024);
        // 1024 × 30 / 30 = 1024 (no alvo, não muda)
        ajustar_lote(&mut t.lote_gravar,ms(30),LOTE_GRAVAR_MIN,LOTE_GRAVAR_MAX);
        assert_eq!(t.lote_gravar,1_024);
        // Validação: começa em 1024 e sobe 2× por tick até o teto de 8192
        ajustar_lote(&mut t.lote_validar,ms(30),LOTE_VALIDAR_MIN,LOTE_VALIDAR_MAX);
        assert_eq!(t.lote_validar,1_024);
        ajustar_lote(&mut t.lote_validar,ms(1),LOTE_VALIDAR_MIN,LOTE_VALIDAR_MAX);
        assert_eq!(t.lote_validar,2_048);
        for _ in 0..10 {
            ajustar_lote(&mut t.lote_validar,ms(1),LOTE_VALIDAR_MIN,LOTE_VALIDAR_MAX);
        }
        assert_eq!(t.lote_validar,8_192);
        // 8192 × 30 / 300 = 819,2 → 819 (queda direta até o alvo)
        ajustar_lote(&mut t.lote_validar,ms(300),LOTE_VALIDAR_MIN,LOTE_VALIDAR_MAX);
        assert_eq!(t.lote_validar,819);
        // 819 × 30 / 1000 = 24,6 → piso 64
        ajustar_lote(&mut t.lote_validar,ms(1000),LOTE_VALIDAR_MIN,LOTE_VALIDAR_MAX);
        assert_eq!(t.lote_validar,64);
    }

    #[test]
    fn conversao_rapida_nao_ignora_aspas_sem_separador() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,r#""São Paulo""#.into());
        app.workbook.escrever_celula(0,1,0,"Ana;25".into());
        app.fim_selecao=(1,0);
        app.assistente_divisao=Some(Estado::novo(&app));
        app.evento_assistente_divisao(Evento::Concluir);
        finalizar_lotes(&mut app);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"São Paulo");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,1),"25");
    }

    #[test]
    fn validacao_conta_colunas_sem_materializar_strings() {
        let o=OpcoesDivisao {
            delimitadores: vec![",".into()],
            consecutivos_como_um: true,
            qualificador_texto: Some('"'),
        };
        for texto in ["", "track_id,artist,album", ",,", "São,,ES",
            r#""São, Paulo",SP"#, "💚,Vitória,2026"] {
            assert_eq!(contar_partes_preparadas(texto,Modo::Delimitado,10,&o),
                partes_preparadas(texto,Modo::Delimitado,10,&o).len());
        }
        assert_eq!(contar_partes_preparadas("a🐦b🔥c",Modo::LarguraFixa,2,&o),3);
    }

    #[test]
    fn separador_preparado_e_largura_fixa_preservam_unicode_sem_buffer_char() {
        let o = OpcoesDivisao {
            delimitadores: vec!["||".into()],
            ..Default::default()
        };
        assert_eq!(
            partes_preparadas("Vitória||ES||2026", Modo::Delimitado, 10, &o),
            ["Vitória", "ES", "2026"],
        );
        assert_eq!(
            partes_preparadas("a🐦b🔥c", Modo::LarguraFixa, 2, &o),
            ["a🐦", "b🔥", "c"],
        );
        assert!(partes_preparadas("", Modo::LarguraFixa, 3, &o).is_empty());
    }

    #[test]
    fn rodape_quatro_etapas_pular_sem_alterar_ate_confirmar() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"João;25;Vitória".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        let _:Element<'_,Msg>=app.view_assistente_divisao();
        app.evento_assistente_divisao(Evento::Avancar);
        assert_eq!(app.assistente_divisao.as_ref().unwrap().etapa,2);
        assert!(app.workbook.valor(0,0,1).is_empty());
        app.evento_assistente_divisao(Evento::IrPara(1));
        app.evento_assistente_divisao(Evento::Pular);
        assert!(app.assistente_divisao.as_ref().unwrap().conversao_em_andamento());
        assert!(app.workbook.valor(0,0,2).is_empty(), "não converte na mesma chamada do clique");
        finalizar_lotes(&mut app);
        assert_eq!(app.workbook.valor(0,0,2),Value::Text("Vitória".into()));
    }
    #[test]
    fn ignorar_coluna_preserva_destino_e_escolher_nova_aba() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Ana;23;Serra".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        app.evento_assistente_divisao(Evento::Coluna(1));
        app.evento_assistente_divisao(Evento::Tipo("Ignorar"));
        app.evento_assistente_divisao(Evento::NovaAba(true));
        app.evento_assistente_divisao(Evento::Concluir);
        assert_eq!(app.workbook.len(),1, "a aba só é criada após validar tudo");
        finalizar_lotes(&mut app);
        assert_eq!(app.workbook.len(),2);
        assert_eq!(app.workbook.valor(1,0,0),Value::Text("Ana".into()));
        assert!(app.workbook.valor(1,0,1).is_empty());
        assert_eq!(app.workbook.valor(1,0,2),Value::Text("Serra".into()));
    }
    #[test]
    fn quatro_etapas_renderizam_sem_scroll_vertical_interno() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"A;12;ES".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        for etapa in 1..=4 {
            app.evento_assistente_divisao(Evento::IrPara(etapa));
            assert_eq!(app.assistente_divisao.as_ref().unwrap().etapa,etapa);
            let _:Element<'_,Msg>=app.view_assistente_divisao();
        }
    }

    #[test]
    fn outro_separador_e_tipo_texto_preservam_zero_inicial() {
        // Um separador por vez: escolher "," desmarca o ";" padrão.
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Ana,001,Serra".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        app.evento_assistente_divisao(Evento::Separador(","));
        app.evento_assistente_divisao(Evento::Coluna(1));
        app.evento_assistente_divisao(Evento::Tipo("Texto"));
        app.evento_assistente_divisao(Evento::Concluir);
        finalizar_lotes(&mut app);
        assert_eq!(app.workbook.valor(0,0,1),Value::Text("001".into()));
        assert_eq!(app.workbook.valor(0,0,2),Value::Text("Serra".into()));
    }

    #[test]
    fn visual_tem_amostra_real_e_exemplo_identificado_quando_vazio() {
        let mut app=App::default();
        let estado=Estado::novo(&app);
        let (demo,exemplo)=app.linhas_divisao(&estado);
        assert!(exemplo);
        assert!(demo[0].1.contains("Nome;Idade;Cidade"));
        app.workbook.escrever_celula(0,0,0,"Ana;25;Vitória".into());
        let (dados,exemplo)=app.linhas_divisao(&estado);
        assert!(!exemplo);
        assert_eq!(dados[0].1,"Ana;25;Vitória");
    }

    #[test]
    fn formato_no_cabecalho_atualiza_apenas_coluna_escolhida() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Ana;001;Vitória".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        assert_eq!(app.assistente_divisao.as_ref().unwrap().destino,"=$A$1");
        app.evento_assistente_divisao(Evento::TipoNaColuna(1,"Texto"));
        assert_eq!(app.assistente_divisao.as_ref().unwrap().selecionada,1);
        assert_eq!(app.assistente_divisao.as_ref().unwrap().tipo(0),TipoColunaDivisao::Geral);
        assert_eq!(app.assistente_divisao.as_ref().unwrap().tipo(1),TipoColunaDivisao::Texto);
        app.evento_assistente_divisao(Evento::Avancado);
        assert!(app.assistente_divisao.as_ref().unwrap().avancado);
        app.evento_assistente_divisao(Evento::Concluir);
        finalizar_lotes(&mut app);
        assert_eq!(app.workbook.valor(0,0,1),Value::Text("001".into()));
    }

    #[test]
    fn cancelamento_apos_lote_gravado_desfaz_tudo() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"A;B".into());
        app.workbook.escrever_celula(0,300,0,"C;D".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        app.evento_assistente_divisao(Evento::Concluir);
        // Cancela após o primeiro lote escrito, antes de concluir a operação.
        // A quantidade de ticks de validação depende do tamanho dos lotes, e
        // a gravação recolhe sem bloquear o que as threads já cortaram: com a
        // máquina ocupada (ou um núcleo só) 10 tiques seguidos terminavam
        // antes de a thread entregar. Espera como o timer do app, com teto.
        for _ in 0..2_000 {
            let _ = app.evento_assistente_divisao(Evento::ProcessarLote);
            if !app.workbook.valor(0,0,1).is_empty() { break; }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
        assert!(app.assistente_divisao.as_ref().unwrap().trabalho.is_some());
        assert_eq!(app.workbook.valor(0,0,1),Value::Text("B".into()));
        app.evento_assistente_divisao(Evento::Cancelar);
        assert_eq!(app.workbook.valor(0,0,0),Value::Text("A;B".into()));
        assert!(app.workbook.valor(0,0,1).is_empty());
        assert_eq!(app.workbook.valor(0,300,0),Value::Text("C;D".into()));
    }

    /// Execução manual em máquina de desenvolvimento: calcula latência real
    /// e RSS do processo; não entra na suíte curta do CI.
    #[test]
    #[ignore = "benchmark de um milhão de linhas: executar manualmente"]
    fn perfil_divisao_um_milhao_linhas_memoria_e_tempo() {
        let mut app=App::default();
        app.workbook.editar(0, |sheet| {
            for linha in 0..1_000_000u32 {
                sheet.set_cell_raw(linha, 0,
                    format!("Nome;{linha};Vitória"), None);
            }
        });
        app.linha=0;
        app.coluna=0;
        app.fim_selecao=(999_999,0);
        app.assistente_divisao=Some(Estado::novo(&app));
        app.evento_assistente_divisao(Evento::Concluir);
        let inicio=std::time::Instant::now();
        let mut iteracoes=0;
        let mut rss_pico=App::rss_atual_kb().unwrap_or(0);
        while app.assistente_divisao.as_ref()
            .is_some_and(Estado::conversao_em_andamento) {
            app.evento_assistente_divisao(Evento::ProcessarLote);
            iteracoes+=1;
            rss_pico=rss_pico.max(App::rss_atual_kb().unwrap_or(0));
            assert!(iteracoes<30_000, "conversor não concluiu os lotes");
        }
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(999_999,0),"Nome");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(999_999,1),"999999");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(999_999,2),"Vitória");
        println!("1M linhas: {:.2}s; pico amostrado {:.1} MiB; {} ticks; {}",
            inicio.elapsed().as_secs_f64(),
            rss_pico as f64 / 1024.0, iteracoes, app.status);
    }

    /// Mede o que roda na thread da janela em cada etapa do Texto para colunas
    /// (nome;cidade;valor). Manual: `--release -- --ignored --nocapture medir_travamento`.
    #[test]
    #[ignore = "benchmark de travamento: executar manualmente"]
    fn medir_travamento_divisao() {
        for linhas in [100_000u32,1_000_000] {
            let mut app=App::default();
            app.workbook.editar(0,|sheet| for l in 0..linhas {
                sheet.set_cell_raw(l,0,format!("Pessoa {l};Vitória;{}",l*3),None);
            });
            app.linha=0; app.coluna=0; app.fim_selecao=(linhas-1,0);
            let t=std::time::Instant::now();
            app.assistente_divisao=Some(Estado::novo(&app));
            let abrir=t.elapsed();
            let t=std::time::Instant::now();
            for _ in 0..10 {
                let e=app.assistente_divisao.as_ref().unwrap();
                let _=app.linhas_divisao(e);
                let _:Element<'_,Msg>=app.view_assistente_divisao();
            }
            let previa=t.elapsed()/10;
            let t=std::time::Instant::now();
            app.evento_assistente_divisao(Evento::Separador(","));
            app.evento_assistente_divisao(Evento::Separador(";"));
            let trocar=t.elapsed()/2;
            let t=std::time::Instant::now();
            app.evento_assistente_divisao(Evento::Concluir);
            let concluir=t.elapsed();
            let (mut max_val,mut max_gra,mut n_val,mut n_gra,mut acima)=(0.0f64,0.0f64,0,0,0);
            let mut ultimo=0.0f64;
            let inicio=std::time::Instant::now();
            while app.assistente_divisao.as_ref().is_some_and(Estado::conversao_em_andamento) {
                let gravando=app.assistente_divisao.as_ref().unwrap()
                    .trabalho.as_ref().is_some_and(|t|t.fase==FaseDivisao::Gravando);
                let t=std::time::Instant::now();
                app.evento_assistente_divisao(Evento::ProcessarLote);
                let ms=t.elapsed().as_secs_f64()*1000.0;
                ultimo=ms;
                if ms>16.0 {acima+=1;println!("  tique lento {ms:.1} ms (gravando={gravando}, n_val={n_val}, n_gra={n_gra})");}
                if gravando {max_gra=max_gra.max(ms);n_gra+=1;} else {max_val=max_val.max(ms);n_val+=1;}
                std::thread::sleep(std::time::Duration::from_millis(4));
            }
            println!("[{linhas} linhas] abrir {:.2} ms | prévia/quadro {:.2} ms | trocar opção {:.2} ms | Concluir {:.2} ms | validar: {n_val} ticks, pior {max_val:.1} ms | gravar: {n_gra} ticks, pior {max_gra:.1} ms | último tique {ultimo:.1} ms | ticks >16 ms: {acima} | total {:.2} s",
                abrir.as_secs_f64()*1000.0,previa.as_secs_f64()*1000.0,
                trocar.as_secs_f64()*1000.0,concluir.as_secs_f64()*1000.0,
                inicio.elapsed().as_secs_f64());
        }
    }

    #[test]
    fn conflito_no_ultimo_lote_impede_toda_gravacao() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"A;B".into());
        app.workbook.escrever_celula(0,5000,0,"C;D".into());
        app.workbook.escrever_celula(0,5000,1,"ocupada".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        app.assistente_divisao.as_mut().unwrap().area=Rect::new(0,0,5000,0);
        app.evento_assistente_divisao(Evento::Concluir);
        finalizar_lotes(&mut app);
        assert!(app.assistente_divisao.as_ref().unwrap().erro.contains("B5001"));
        assert_eq!(app.workbook.valor(0,0,0),Value::Text("A;B".into()));
        assert!(app.workbook.valor(0,0,1).is_empty());
    }

    #[test]
    fn destinos_invalidos_e_conflito_nao_modificam_a_planilha() {
        assert_eq!(endereco("=$B$12"),Some((11,1)));
        assert_eq!(endereco("XFE1"),None);
        assert_eq!(endereco("A0"),None);
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"A;B".into());
        app.workbook.escrever_celula(0,0,1,"Ocupada".into());
        app.assistente_divisao=Some(Estado::novo(&app));
        app.evento_assistente_divisao(Evento::Concluir);
        finalizar_lotes(&mut app);
        assert!(app.assistente_divisao.as_ref().unwrap().erro.contains("B1"));
        assert_eq!(app.workbook.valor(0,0,0),Value::Text("A;B".into()));
    }
}
