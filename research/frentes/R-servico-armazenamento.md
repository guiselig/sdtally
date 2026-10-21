# Frente R: chamar o StorageManagementService direto (19/10)

Máquina: macOS 27.0 (26A5416b), M4, SIP ligado, usuário admin, sem sudo, um certificado "Apple
Development" gratuito (conta pessoal, sem Developer Program). Método: leitura estática do serviço
(`plutil`, `codesign`, `dyld_info -objc`, `otool`, desmontagem), clientes de teste em ObjC
compilados e rodados aqui, e `log stream`/`log show` (predicates de serviço, `amfid`, `kernel`,
`tccd`, `pkd`). Nada do sistema foi alterado: sem sudo, sem `launchctl` de escrita, sem tocar em
TCC/SIP, Ajustes nunca aberto. Código de produção intocado.
Grau por item: **A** visto/medido aqui, **M** inferência forte, **B** hipótese ou fonte externa.

## 1. Resultado

**Veredito: inviável.** Um app assinado só com Apple Development gratuito não consegue chamar o
`StorageManagementService`, com ou sem Acesso Total ao Disco (FDA), com ou sem root.

- O serviço só aceita cliente que carregue o entitlement `com.apple.storage-data`. A função
  `-[ServiceDelegate listener:shouldAcceptNewConnection:]` faz `valueForEntitlement:` e, se vier
  nulo, registra `Client N is missing entitlements required to use this service` e derruba a
  conexão. Reproduzido: conexão recusada, nenhum cálculo iniciado (A).
- Esse entitlement é **restrito**: o `amfid` mata qualquer binário que o declare, ad hoc
  (`-424 adhoc signed but contains restricted entitlements`) ou com o Apple Development gratuito
  (`-413 No matching profile found`). Só um perfil de provisionamento emitido pela Apple poderia
  autorizá-lo, e a chave não é documentada nem é capability do portal (A para o bloqueio; M para
  "nem o Developer Program pago dá"; ver §6).
- **FDA não muda nada.** O processo que roda os testes tem FDA, e o binário de teste filho herdou
  (leu `~/Library/Mail`). Mesmo assim o serviço recusou. O portão é entitlement de assinatura,
  não TCC (A, §4).
- **Nada precisa ser configurado pelo usuário, porque nada adianta configurar.** Não há item em
  Privacidade e Segurança, Itens em Segundo Plano, root ou entitlement acessível que destrave a
  chamada (§5).
- **Caminho alternativo também fechado:** hospedar as extensões `com.apple.storagemanagement`
  pelo nosso app (para pedir `totalSpaceUsed` de cada categoria) é recusado pelo `pkd`, que exige
  outro entitlement restrito, `com.apple.developer.extension-host.systeminfoapp` (A).

**O que dá para fazer sem abrir o Ajustes (achado novo, §7):** o número do serviço aparece no log
mesmo sem ninguém abrir o painel. Um daemon da Apple (`ASPCarryLog`, root, tem o entitlement)
chama o serviço sozinho numa atividade semanal (`com.apple.nand.nandtelemetry_appspace`) e o
serviço grava o bloco `StorageLogInvestigation` completo em ~85 s. Observado em 18/10 às 21:26,
20 min antes de o painel ser aberto. É oportunista (condições da atividade, §7), mas é a única via
"sem o painel" que entrega o número do Ajustes, e a leitura do log que o app já faz (`tela_log`)
a cobre sem mudança.

## 2. O portão, em código

| Peça | Fato | Grau |
|---|---|---|
| LaunchAgent `/System/Library/LaunchAgents/com.apple.StorageManagement.Service.plist` | `MachServices: com.apple.StorageManagement.Service`, `LimitLoadToSessionType: Aqua`, `EnableTransactions`, `EnablePressuredExit`, binário em `StorageManagement.framework/PlugIns/StorageManagementService`. Sob demanda: o primeiro `connect` o inicia (`launchctl print gui/$UID/...`: `state = not running` antes, `runs` 35 → 36 depois dos testes) | A |
| Entitlements do serviço | `com.apple.private.tcc.allow` = `kTCCServiceSystemPolicyAllFiles, Photos, AddressBook, Calendar, Reminders, AppleEvents, PostEvent`; `com.apple.developer.extension-host.systeminfoapp`; `com.apple.private.iaaccounts`; `com.apple.security.scripting-targets` (Finder/trash). Assinado "macOS Software Signing", `Platform identifier=26` | A |
| Sandbox do serviço | `main` chama `sandbox_init_with_parameters("com.apple.StorageManagement.Service", ...)` (perfil nomeado, não lido) | A |
| Aceite da conexão | `-[ServiceDelegate listener:shouldAcceptNewConnection:]` em `0x10000CF2C` (`dyld_info -objc`). Carrega o `CFString` `@"com.apple.storage-data"` (`0x100031D98`), faz `objc_msgSend(conn, valueForEntitlement:)` (selref `0x10003A6F8` resolvido para `valueForEntitlement:`), `cbz` no resultado; nulo → `NSLog(@"Client %d is missing entitlements required to use this service", pid)` e retorna NO. Não vi comparação de valor, só presença | A (fluxo) / B (só presença) |
| Interface exportada | `STMServiceProtocol` (12 métodos): `startMonitoringWithCompletionHandler:`, `stopMonitoring`, `setMainReplyHandler:`, `getDetailForExtensionIdentifier:completionHandler:`, `enableRecommendations:`, `setRecommendationsReplyHandler:`, `performRecommendationWithIdentifier:`, `performCancelRecommendationActionWithIdentifier:`, `performFilesActionForExtensionIdentifier:`, `performSummaryActionForExtensionIdentifier:`, `performRevealFiles:extensionIdentifier:`, `performDeleteFiles:extensionIdentifier:`. O resultado volta por push no cliente, via `STMServiceMainReplyHandler`: `sendSidebarItems:`, `sendVolumeCategories:forVolumeMountPath:`, `sendDisksList:`, `sendOpticalDisksList:`, `sendExcludedURLs:`, `presentError:`, `presentAlert:confirmationHandler:`. Não existe método "me dê o número" síncrono | A (runtime ObjC) |
| Cliente de biblioteca | `STMTelemetryController` (`getTelemetryWithCompletionHandler:`, `retrieveTelemetryAfterQuiesceDelay:completionHandler:`) e `STMXPCInterfaceFactory +interface`, ambos no `StorageManagement.framework` (cache do dyld, `dlopen` funciona) | A |
| Quem tem `com.apple.storage-data` no disco | `grep -rl` em `/System/Applications`, `CoreServices`, `ExtensionKit`, `PrivateFrameworks`, `Frameworks`, `/usr/{bin,sbin,libexec}`, `/bin`, `/sbin`: só `Storage.appex`, o próprio serviço (string) e `/usr/libexec/ASPCarryLog` (daemon de telemetria NAND/disco). Nenhuma ferramenta de linha de comando serve de intermediária | A |
| Entitlements do `Storage.appex` | `com.apple.storage-data`, `com.apple.private.tcc.allow` (mesma lista do serviço), `com.apple.private.followup`, `com.apple.private.security.no-sandbox`, `com.apple.private.security.storage.MobileAssetGenerativeModels`, `com.apple.security.sandbox=false` | A |
| Entitlements das extensões (`PlugIns/*.appex`) | Todas sandbox + `com.apple.private.tcc.allow` próprio + exceções (`mach-lookup`, `sbpl`, `shared-preference`). Específicos: Applications (`MobileContainerManager.allowed`, `librarian.*`), Outros usuários (`systemmigration.daemonclient`), iOS (`security.storage.mobilesync`), CloudDocs (`clouddocs.storage-management`, `fileprovider.storage-management`), Música (`amp.library.client`, `assets.music.read-only`). Cada uma carrega o seu TCC por entitlement: o cliente nunca é consultado | A |
| Agentes vizinhos | `com.apple.diskspaced` (alerta de pouco espaço) e `com.apple.STMUIHelper` (diálogos): LaunchAgents Aqua, sem interface de dados útil | A (plist) / M (papel) |

## 3. Evidências dinâmicas (comando → saída → grau)

Clientes de teste em `build-svc/` (fora do repo, fonte no apêndice). Todos rodados deste shell.

| # | Comando | Saída resumida | Grau |
|---|---|---|---|
| 1 | `./cli1 raw` (NSXPCConnection puro com `STMXPCInterfaceFactory.interface`, `startMonitoringWithCompletionHandler:`), binário ad hoc, sem entitlement | Cliente: `INTERRUPTED` e `proxy error: NSCocoaErrorDomain 4097 "connection to service named com.apple.StorageManagement.Service"`. Serviço (log, 00:13:55): `Client 96019 is missing entitlements required to use this service`, `invalidated because the current process cancelled the connection by calling xpc_connection_cancel()` | A |
| 2 | `./cli1 tel` (`STMTelemetryController getTelemetryWithCompletionHandler:`), mesma assinatura | Cliente: `[com.apple.storage:log] Start service`, `Error caught talking to telemetry service: ... 4097`, `Could not start service`, `_callCompletionWithResults: NO`. Serviço: `Client 96062 is missing entitlements ...` | A |
| 3 | `codesign -f -s - --entitlements ent_sd.plist` (só `com.apple.storage-data`) e executar | Exit 137. `amfid`: `Adhoc signed app with restricted entitlements detected`, `AppleMobileFileIntegrityError Code=-424 "The file is adhoc signed but contains restricted entitlements"`. Kernel: `Code has restricted entitlements, but the validation of its code signature failed` | A |
| 4 | Idem com `-s "Apple Development: ..."` (certificado gratuito, TeamIdentifier presente) | Exit 137. `amfid`: `Failure validating against provisioning profiles`, `Code=-413 "No matching profile found"`; kernel `load code signature error 4` | A |
| 5 | Sondagem de hospedagem: `NSExtension extensionWithIdentifier:@"com.apple.STMExtension.Trash" error:` num binário comum | Retorna nulo. `pkd`: `rejecting; plugin disallowed for client: the host does not have the "com.apple.developer.extension-host.systeminfoapp" entitlement` | A |
| 6 | Idem 5, assinado com Apple Development gratuito + `com.apple.developer.extension-host.systeminfoapp` | Exit 137, `amfid` `Code=-413 "No matching profile found"` | A |
| 7 | Controle positivo: `log show` de 18/10 (processo `StorageManagementService`, peers) | Aceitou `peer[28939]` (`ASPCarryLog`, 21:26:10) e `peer[41852]` (`Storage.appex`, 21:47:52) e calculou para os dois. Prova que o serviço funciona e que a diferença é só a assinatura do cliente | A |
| 8 | `log show --last 30d` com `subsystem == "com.apple.storage" AND eventMessage == "Start service"` | Um cliente legítimo (ASPCarryLog, 18/10 21:26:09) e 1 do nosso teste (`cli1 tel`; o modo `raw` não usa a biblioteca e não grava essa linha). O log desta máquina começa em 05/10 | A |
| 9 | Efeito dos nossos testes no serviço | Nenhuma linha `StorageLogInvestigation` entre 00:13 e 00:17 (recusa antes de qualquer trabalho). O serviço iniciado ficou residente: 7 min depois, 0,05 s de CPU, 1,4 a 1,8 MB de RSS, ainda `running` | A |
| 10 | Páginas públicas | Nenhum header, documentação ou ferramenta pública do protocolo (`STMServiceProtocol`, `STMXPCInterfaceFactory`, `STMStorageController`: zero resultados). Ver §8 | B |

## 4. O cenário "cliente com FDA concedido" (premissa nova do dono)

1. **O processo que roda estes testes já tem FDA, e filho recém-compilado herda.** `ls ~/Library/Mail`
   e `~/Library/Safari` listam; `fdaprobe` (C, ad hoc, nunca autorizado em Privacidade) faz
   `opendir("~/Library/Mail")` com sucesso (4 entradas). Cadeia: `claude.app` (pid 21964,
   pai do `zsh`) → `zsh` → binário. Logo os testes 1 a 4 e 7 acima já foram feitos
   **com FDA efetivo**, e o serviço recusou igual (A). O `tccd` não registrou pedido para o
   `fdaprobe` (resposta já em cache do kernel/sandbox), então o "responsável" não ficou provado no
   log do `tccd`; ficou provado pelo efeito (A para o efeito, M para o mecanismo).
2. **O TCC do serviço não depende do cliente.** O serviço carrega `kTCCServiceSystemPolicyAllFiles`
   por `com.apple.private.tcc.allow`, e cada extensão também. Se uma chamada fosse aceita, o acesso
   a `~/Library`, Fotos, Contatos etc. seria atribuído ao próprio serviço/extensão, sem prompt e sem
   exigir FDA do nosso app (A pelo plist; M pela atribuição, já que não deu para ver uma chamada
   aceita).
3. **O que FDA do cliente não cobre:** o portão `valueForEntitlement:` roda antes de qualquer
   consulta TCC, e o `amfid` não olha TCC. FDA é irrelevante para os dois bloqueios.
4. **Além do FDA, o que seria exigido:** somente o entitlement privado `com.apple.storage-data` no
   cliente, que só um perfil emitido pela Apple autoriza. Não exigiria Arquivos e Pastas, Itens em
   Segundo Plano nem root (§5).
5. **FDA continua útil para o resto do produto**: é o que alimenta a varredura própria
   (Frentes D, P, Q). Só não abre esta porta.

## 5. O que cada permissão/configuração mudaria

| Item | Resolve? | Por quê | Grau |
|---|---|---|---|
| Acesso Total ao Disco | Não | Portão é entitlement de assinatura; testes 1 a 4 rodaram com FDA herdado | A |
| Arquivos e Pastas / prompts do TCC | Não, e nem aparecem | O cliente nunca chega a pedir arquivo; o serviço usa o TCC embutido dele | A (portão) / M (atribuição) |
| Itens em Segundo Plano / Login Items | Não | Não é questão de registrar nosso app. O serviço é LaunchAgent do sistema (Apple, `/System/Library/LaunchAgents`), sob demanda; nenhuma tela de aprovação foi acionada nos testes | A (teste) / M (BTM não consultado: `sfltool` pede root) |
| Rodar como root | Não | O `amfid` avalia assinatura + perfil, não UID; e o serviço é agente da sessão Aqua do usuário (`LimitLoadToSessionType: Aqua`) | M (sem sudo para testar; o veredito do `amfid` não tem campo de UID) |
| Desligar SIP / `amfi_get_out_of_my_way` | Faria funcionar, mas fora de questão | Proibido aqui e inaceitável como requisito de produto | B (não testado) |
| Entitlement `com.apple.storage-data` no nosso app | Sim, mas inobtível | Restrito: `-413 No matching profile found` com Apple Development gratuito | A |
| Apple Developer Program pago | Provavelmente não | A chave não consta como capability de App ID nem na documentação de entitlements; `com.apple.*` privados só entram em perfil por concessão da Apple | B |
| Notarização / Developer ID | Não | Notarizar não concede entitlement restrito | M |
| App com bundle ou binário cru | Indiferente | O bloqueio é igual nos dois (binário cru nos testes 1 a 6; um `.app` assinado com o mesmo certificado herda o mesmo `amfid`) | A (cru) / M (.app) |

## 6. Incertezas desta parte

- **Paid account.** Não há conta paga aqui para tentar gerar um perfil com `com.apple.storage-data`.
  A conclusão "inobtível" para o Developer Program vem da regra do `amfid` (perfil precisa conter a
  chave) e da ausência dela entre as capabilities públicas. Para confirmar sem risco, o dono pode
  abrir o portal (Certificates, Identifiers & Profiles, App ID, Capabilities) e procurar por
  "storage"; nada nesse sentido existe nos relatos públicos que achei (B).
- **Perfil de Xcode com Personal Team.** Não gerei um `embedded.provisionprofile` (exigiria uma
  ação na conta da Apple). O erro `-413` diz que perfil nenhum bateu. O Xcode gratuito recusa
  entitlement fora da lista de capabilities (M).
- **Checagem só de presença** (não de valor): vi `cbz` no retorno, não li o corpo do tratamento
  do valor (B). Irrelevante, já que não dá para ter a chave.
- **Como um daemon root alcança um agente de usuário.** `ASPCarryLog` está no domínio `system` e o
  serviço em `gui/501`. O log mostra a conexão aceita (peer 28939), mas não o mecanismo de
  lookup entre domínios. Não afeta o veredito (C).
- Build única (26A5416b). A fonte externa da §8 viu o mesmo comportamento em 27.2 (26B5091g), o que
  sugere estabilidade entre betas (B).

## 7. Achado lateral: o serviço roda sozinho, sem o painel

Pelo log de 18/10 havia duas sessões distintas, não uma:

| Sessão | Cliente | Início | Último bloco | Observação |
|---|---|---|---|---|
| 1 | `ASPCarryLog` pid 28939 (daemon `com.apple.nand.aspcarry`, root, domínio `system`), atividade `com.apple.nand.nandtelemetry_appspace` | 21:26:09.854 (`Start service`) | 21:27:34.054 | 45 blocos (36 transitórios com `totalSpace = 0`, 9 válidos); 10 extensões respondidas aos ~46 s (21:26:55); Other 204,04 GB → 204,27 GB (21:27:34); identidade `Used − System − Σext == Other` e `total − avail − otherVols == Used` verdadeira nos 45 |
| 2 | `Storage.appex` pid 41852 (painel aberto pelo dono) | 21:47:52 | 21:51:39 | 89 blocos (64 válidos, identidade verdadeira em todos); Other 196,52 GB; serviço sai 21:51:42 |

- A atividade está no plist `/System/Library/LaunchDaemons/com.apple.nand.aspcarry.plist`:
  `Interval 604800` (semanal), `Delay 3600`, `GracePeriod 43200`, `Repeating`,
  **`AllowBattery false`, `RequireScreenSleep true`**. Ou seja, só roda com o Mac na tomada e a
  tela em repouso, e o sistema escolhe o momento (A para o plist e para a sessão de 18/10).
- Só uma execução apareceu nos 14 dias de log disponíveis (05/10 a 19/10). Semanal "puro" daria
  duas; não sei por que só uma (Mac em bateria, tela ativa, atividade adiada) (B).
- O app não controla esse gatilho (não há como rodar a atividade sem `launchctl`/root). O que o app
  pode fazer é ler o último bloco válido no log, qualquer que seja o cliente que o provocou.
  A coleta atual (`painel.rs`, predicate `subsystem == "com.apple.storage" AND eventMessage
  BEGINSWITH "StorageLogInvestigation - "`) já cobre as duas origens. O que falta é a interface
  dizer a origem e a idade ("calculado pelo sistema em 18/10 21:27, ~85 s de cálculo") e nunca
  tratar bloco com `totalSpace == 0` como válido (o primeiro bloco da sessão 1 tem
  `Other = −94,9 GB` e `totalSpace = 0`).
- Custo da via do sistema: 85 s de relógio do serviço (sessão 1) e 3 min 47 s (painel, que fica
  recalculando). CPU e memória do serviço em cálculo **não medi** (§9).
- Consequência de produto: dá para prometer "número do próprio Ajustes, sem abrir o painel, atrasado
  até ~1 semana quando as condições da atividade não forem satisfeitas". Não dá para prometer
  "agora" sem o painel.

## 8. Fontes externas

- Pesquisa pública não encontrou headers, ferramentas nem artigo sobre `STMServiceProtocol`,
  `STMTelemetryController` ou `StorageLogInvestigation`; o repositório de headers
  [w0lfschild/macOS_headers](https://github.com/w0lfschild/macos_headers/) e a lista de
  [LeoNatan/Apple-Runtime-Headers](https://github.com/LeoNatan/Apple-Runtime-Headers) são do tipo
  que traria isso, mas não tive resultado para esses nomes.
- [MacSpace, Docs/Research.md](https://github.com/1architect/macspace-releases/blob/main/Docs/Research.md)
  (analisa o mesmo serviço em macOS 27.2): "Nothing here can be asked from outside: the service
  answers only clients with `com.apple.storage-data`", e o log do painel só tem tamanhos em nível
  debug. Chega à mesma fórmula (`Dados do Sistema = usado − macOS − Σ categorias`). Confirma
  independentemente o veredito (B, fonte de terceiro).
- Man page local `StorageManagementService(8)`: "system daemon backing the Storage section"; sem
  opção, "users should not run it manually". Já citada na Frente A.

## 9. O que falta e como o dono confere

1. **Reproduzir a recusa** (2 min, nada é alterado): compilar o apêndice
   (`clang -fobjc-arc -framework Foundation cli1.m -o cli1`), rodar `log stream --predicate
   'process == "StorageManagementService"'` em outro terminal e executar `./cli1 raw 8`. Esperado:
   a linha `Client N is missing entitlements required to use this service`.
2. **Confirmar a via passiva do sistema** (7 dias, sem abrir o Ajustes): deixar o Mac na tomada com
   a tela apagada algumas noites e depois rodar
   `/usr/bin/log show --last 7d --style compact --info --debug --predicate 'subsystem == "com.apple.storage" AND eventMessage == "Start service"'`.
   Cada linha cujo processo seja `ASPCarryLog` é uma sessão automática; para ler o bloco, a mesma
   consulta da Frente A com `eventMessage BEGINSWITH "StorageLogInvestigation - "` no intervalo.
   Isso diz quantas vezes por semana a via passiva realmente rende.
3. **Comparar com a tela, ao byte** (precisa do dono, uma vez): o bloco das 21:27 de 18/10 não
   tem print correspondente, então não posso afirmar qual dos dois bate com o gauge em cada
   instante. Passo exato: abrir Ajustes > Geral > Armazenamento e esperar o painel parar de
   recalcular (~4 min); então rodar a consulta da Frente A para o último bloco e comparar com o
   que o gauge mostra. Já existe a rotina da Frente P §5 item 1 para isso.
4. **CPU e memória do serviço em cálculo:** durante o passo 3, em outro terminal,
   `while true; do ps -o time,%cpu,rss -p $(pgrep StorageManagementService); sleep 5; done`.
   Fecha o item 4 do briefing (tempo e custo).
5. **Developer Program pago** (opcional, se o dono quiser certeza total): conferir no portal se existe
   capability ou entitlement `com.apple.storage-data`. Se aparecer, reabrir esta frente.

## 10. Riscos

- Apple pode mudar o texto/nível do log ou parar de chamar o serviço pela telemetria; a via passiva
  não tem contrato (B). A coleta já valida a identidade aritmética antes de exibir.
- Os testes deixaram o serviço residente por alguns minutos (custo desprezível, §3 item 9) e geraram
  um relatório de falha de um binário de teste meu em
  `~/Library/Logs/DiagnosticReports/cli1-2026-10-19-001408.ips` (bug do teste, corrigido; o
  arquivo pode ser apagado pelo dono).

## Apêndice: cliente de teste (`cli1.m`)

```objc
// Modo "tel": STMTelemetryController. Modo "raw": NSXPCConnection puro.
#import <Foundation/Foundation.h>
#import <objc/runtime.h>
#import <objc/message.h>
#import <dlfcn.h>

@interface Handler : NSObject
@end
@implementation Handler
- (void)sendDisksList:(id)x { printf("[reply] sendDisksList\n"); }
- (void)sendExcludedURLs:(id)x { printf("[reply] sendExcludedURLs\n"); }
- (void)sendOpticalDisksList:(id)x { printf("[reply] sendOpticalDisksList\n"); }
- (void)sendSidebarItems:(id)x { printf("[reply] sendSidebarItems\n"); }
- (void)sendVolumeCategories:(id)c forVolumeMountPath:(id)p { printf("[reply] sendVolumeCategories\n"); }
- (void)presentError:(id)e { printf("[reply] presentError\n"); }
- (void)presentAlert:(id)a confirmationHandler:(id)h { printf("[reply] presentAlert\n"); }
@end

int main(int argc, char **argv) {
  @autoreleasepool {
    const char *mode = argc > 1 ? argv[1] : "tel";
    dlopen("/System/Library/PrivateFrameworks/StorageManagement.framework/StorageManagement", RTLD_NOW);
    __block BOOL done = NO;
    if (!strcmp(mode, "tel")) {
      Class C = NSClassFromString(@"STMTelemetryController");
      id t = ((id(*)(id,SEL))objc_msgSend)([C alloc], @selector(init));
      void (^cb)(BOOL) = ^(BOOL ok) { printf("[tel completion] ok=%d\n", ok); done = YES; };
      ((void(*)(id,SEL,id))objc_msgSend)(t, NSSelectorFromString(@"getTelemetryWithCompletionHandler:"), cb);
    } else {
      NSXPCConnection *c = [[NSXPCConnection alloc] initWithMachServiceName:@"com.apple.StorageManagement.Service" options:0];
      Class F = NSClassFromString(@"STMXPCInterfaceFactory");
      c.remoteObjectInterface = ((id(*)(id,SEL))objc_msgSend)(F, NSSelectorFromString(@"interface"));
      c.exportedInterface = [NSXPCInterface interfaceWithProtocol:NSProtocolFromString(@"STMServiceMainReplyHandler")];
      c.exportedObject = [Handler new];
      c.invalidationHandler = ^{ printf("[raw] INVALIDATED\n"); done = YES; };
      c.interruptionHandler = ^{ printf("[raw] INTERRUPTED\n"); };
      [c resume];
      id p = [c remoteObjectProxyWithErrorHandler:^(NSError *e){ printf("[raw] proxy error: %s\n", [[e description] UTF8String]); }];
      ((void(*)(id,SEL,id))objc_msgSend)(p, NSSelectorFromString(@"startMonitoringWithCompletionHandler:"), ^{ printf("[raw] startMonitoring completion\n"); });
    }
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow: argc > 2 ? atof(argv[2]) : 15];
    while (!done && [end timeIntervalSinceNow] > 0) [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.2]];
    printf("exit done=%d\n", done);
  }
}
```

Para os testes 3 e 4: `codesign -f -s <identidade> --entitlements ent.plist <cópia do cli1>` com
`ent.plist` contendo só `com.apple.storage-data = true`; `-s -` para ad hoc.
