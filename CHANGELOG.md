# Changelog

## [0.6.0](https://github.com/magnus919/SlopSearX/compare/v0.5.0...v0.6.0) (2026-09-22)


### Features

* add optional Jev specialist routing ([#399](https://github.com/magnus919/SlopSearX/issues/399)) ([dfa12ab](https://github.com/magnus919/SlopSearX/commit/dfa12aba6c3cb359dc973865df12f97b931f01b8))
* expose Jev routing across MCP portal and agent surfaces ([1eff141](https://github.com/magnus919/SlopSearX/commit/1eff14116068d9193aaba596a23e1fa9ca6cbd4d))
* expose Jev routing across MCP portal and agent surfaces ([3d3e6f2](https://github.com/magnus919/SlopSearX/commit/3d3e6f23a4282020021901f44eb0e917629e4503))


### Bug Fixes

* accept filtered Exploit-DB empty results ([426498a](https://github.com/magnus919/SlopSearX/commit/426498a676ae72b56bb608195cdb752167171334))
* align OAuth types and FastMCP imports ([a592152](https://github.com/magnus919/SlopSearX/commit/a5921522da26f13616dd5298a2ecd68ff0e4c6a7))
* allow public GitHub repository and issue search ([2895885](https://github.com/magnus919/SlopSearX/commit/2895885fb365fcb215d56c0089b40628cb241404))
* allow public GitHub repository and issue search ([c87a60d](https://github.com/magnus919/SlopSearX/commit/c87a60d8383fb17a34a5b4e8c359b3eb604d32c4))
* bound PubMed aggregate requests ([6a4f1e4](https://github.com/magnus919/SlopSearX/commit/6a4f1e43e4800cf2efb4789db8a6f1027c2042e6))
* bound PubMed aggregate timeout ([ac62427](https://github.com/magnus919/SlopSearX/commit/ac624277da98018c5f6ccc0caab8f95102a96f27))
* classify crt.sh challenge responses ([5df6545](https://github.com/magnus919/SlopSearX/commit/5df6545683320c462450ed223725c557d69e1d1b))
* classify malformed crt.sh responses ([bf11607](https://github.com/magnus919/SlopSearX/commit/bf116075b7fe9dc68b3dcf8028fac31226c03189))
* classify malformed crt.sh responses ([226f310](https://github.com/magnus919/SlopSearX/commit/226f310f7df89474ac4822e1210c50ddd03dff64))
* classify malformed GreyNoise responses ([f9a7e4e](https://github.com/magnus919/SlopSearX/commit/f9a7e4e721ccb045535f2dec268e4a995b923953))
* classify URLhaus auth and no-match responses ([93001e4](https://github.com/magnus919/SlopSearX/commit/93001e40926603dd1e39016282a026188bacfed2))
* classify URLhaus auth and no-match responses ([7ca3219](https://github.com/magnus919/SlopSearX/commit/7ca321945c3029f3a30c0719d3118bc147b98f3b))
* classify Wikipedia provider failures ([409dcf0](https://github.com/magnus919/SlopSearX/commit/409dcf0823b246836a2106f676f087d5a08ed81a))
* classify Wikipedia provider failures ([f79b09c](https://github.com/magnus919/SlopSearX/commit/f79b09ce41d40240d0fc3516beec25bf0f5a8506))
* complete FastMCP v4 transport compatibility ([e214f32](https://github.com/magnus919/SlopSearX/commit/e214f32a2edb584acf67a993a96ccfab83aee264))
* cool down Semantic Scholar after 429 ([125b4de](https://github.com/magnus919/SlopSearX/commit/125b4de9243cdcf17ae0929b22a5d649c8bdb4bf))
* cool down Semantic Scholar after 429 ([0c82916](https://github.com/magnus919/SlopSearX/commit/0c82916a41350e26a933679313138f3f75f57782))
* distinguish Exploit-DB challenge and empty pages ([2122210](https://github.com/magnus919/SlopSearX/commit/21222108092ccf69eeaa16c1de6eb272d4f064ed))
* distinguish Exploit-DB challenge and empty pages ([3b45f72](https://github.com/magnus919/SlopSearX/commit/3b45f729d5be50b11cdc7df27a4098f43e8a320d))
* distinguish generic ATS queries from empty boards ([624cee4](https://github.com/magnus919/SlopSearX/commit/624cee40e6be098a1625445c85e6518270a8c312))
* distinguish generic ATS queries from empty boards ([24f8b0d](https://github.com/magnus919/SlopSearX/commit/24f8b0dd9035dd339e8b417e8ecf9eb6d315b6a0))
* distinguish GreyNoise empty and malformed responses ([252538d](https://github.com/magnus919/SlopSearX/commit/252538dfe63eaa76bcd0e569995bd9a3ca9aa8f1))
* enforce PubMed deadline after parsing ([3323014](https://github.com/magnus919/SlopSearX/commit/33230145317cbb6deb24875ce388645d5b497137))
* fall back to Wayback Availability API ([055f07c](https://github.com/magnus919/SlopSearX/commit/055f07c5fc1ad8dad2172dadd9beb373d5796413))
* fall back to Wayback Availability API ([e7853dc](https://github.com/magnus919/SlopSearX/commit/e7853dcae92692d1ed7b1d587d8fe8def8101fb8))
* harden EDGAR result URLs ([c4864cd](https://github.com/magnus919/SlopSearX/commit/c4864cde033c6cda1f978119b2aaeb25f5a99f87))
* harden EDGAR result URLs ([ac57f91](https://github.com/magnus919/SlopSearX/commit/ac57f91062b5055dd457710799b8d6235e7ed7ea))
* honor research completion replay and architecture layers ([ebdab9a](https://github.com/magnus919/SlopSearX/commit/ebdab9ae212d76cbf886154783c9bc3b5b3f0adc))
* improve arXiv phrase relevance ([39d1401](https://github.com/magnus919/SlopSearX/commit/39d1401e7ca369a200e33a78e75b0d90ea9f5044))
* improve openFDA relevance and result identity ([91a3da6](https://github.com/magnus919/SlopSearX/commit/91a3da6e2682036df583268908f8da60f834e83e))
* improve openFDA relevance and result identity ([8f1a555](https://github.com/magnus919/SlopSearX/commit/8f1a55508787fb6bc22605ac1b86d9340a269573))
* improve Oyez named-case relevance ([26437c6](https://github.com/magnus919/SlopSearX/commit/26437c639313b3a29036ec1add1a9320f20294b8))
* isolate Exploit-DB JSON challenge handling ([741d704](https://github.com/magnus919/SlopSearX/commit/741d7040cd3a64fd32f456c3c2b722372b5292a3))
* make FastMCP HTTP stateless for v4 compatibility ([9dc787e](https://github.com/magnus919/SlopSearX/commit/9dc787e96df24fec2b1fbff901f2f5f52b185c93))
* make oauth provider FastMCP v4 compatible ([e91fbb8](https://github.com/magnus919/SlopSearX/commit/e91fbb881c9eb13237c12ab4c7b8a670787981fc))
* mitigate Reddit blocked search ([8dbbd12](https://github.com/magnus919/SlopSearX/commit/8dbbd129a751805df9d60a43c67740b8bbf2d0a7))
* mitigate Reddit blocked search ([47c0c9b](https://github.com/magnus919/SlopSearX/commit/47c0c9b28f3330976583be8250f140f3f59f3fb2))
* pass engine credentials through compose ([606d695](https://github.com/magnus919/SlopSearX/commit/606d695515a400e9a50b18d5cfb9ef0e177e7fc5))
* persist exact query budget exhaustion ([7016d2c](https://github.com/magnus919/SlopSearX/commit/7016d2cfc63b857b255ab5d32b4373fa74387a30))
* preserve execution outcome on caller completion ([92cd0e4](https://github.com/magnus919/SlopSearX/commit/92cd0e4c3aeee91a12c61d26411759e7b276d97a))
* preserve execution outcome on caller completion ([0b6d285](https://github.com/magnus919/SlopSearX/commit/0b6d28559e666c3c4650d4f51dcabe30c2bcf07d))
* preserve explicit arXiv query syntax ([b7773b2](https://github.com/magnus919/SlopSearX/commit/b7773b28f2d46ff97218e22afcfa34d6772fe5e0))
* preserve Jev router in HTTP search context ([604c611](https://github.com/magnus919/SlopSearX/commit/604c6113a256ae6f8e1de127550c86f9f2660ed2))
* preserve Open Library result identity URLs ([9a58a42](https://github.com/magnus919/SlopSearX/commit/9a58a428a16fafcf5b12ea86f627f3b1dc0265d9))
* preserve Open Library result identity URLs ([75c9df5](https://github.com/magnus919/SlopSearX/commit/75c9df51ac64805b2274628e71f5f78f3a349c8b))
* prioritize exact UniProt matches ([53dbafd](https://github.com/magnus919/SlopSearX/commit/53dbafd9207eed175228c594058af85ed8e9bd0e))
* prioritize exact UniProt matches ([11c6494](https://github.com/magnus919/SlopSearX/commit/11c649491110743d26a1820d229a37de2fa997ad))
* query current Exploit-DB JSON endpoint ([3446145](https://github.com/magnus919/SlopSearX/commit/34461458618c3fa0078ac0b7ac97931c60cf899e))
* report default English filter enforcement in MCP ([#392](https://github.com/magnus919/SlopSearX/issues/392)) ([55887e6](https://github.com/magnus919/SlopSearX/commit/55887e6923050f3467566af6b60d6e19589aacfe))
* retain SDK server lifecycle for FastMCP 3 ([ed971b2](https://github.com/magnus919/SlopSearX/commit/ed971b2296712cfb3708b65114d052681567b835))
* retire Repology engine ([af150d2](https://github.com/magnus919/SlopSearX/commit/af150d259c5fae8a4bd452a909f8a4dbcaab6e06))
* retire Repology engine ([862afd3](https://github.com/magnus919/SlopSearX/commit/862afd3739e607bf08c6ff762acad602ead4633c))
* scope Exploit-DB challenge detection ([983b34d](https://github.com/magnus919/SlopSearX/commit/983b34de1229830a10f1197ef9e2d03fa64128ec))
* support FastMCP v4 gateway clients ([ad79479](https://github.com/magnus919/SlopSearX/commit/ad79479928befb6c3824373d3a7ba96e8b3eeeb9))
* support FastMCP v4 server transport ([f13c41e](https://github.com/magnus919/SlopSearX/commit/f13c41ec1bc9b6c80097d707476c71886de52b09))
* support FastMCP v4 transports and OAuth ([63e3ecd](https://github.com/magnus919/SlopSearX/commit/63e3ecd2f09d79c34e3594a3be74f017a1c5a12c))
* target Docker Hub namespace and exact repository names ([8234d0b](https://github.com/magnus919/SlopSearX/commit/8234d0b907778c2b076d5fd8137a46655a9cf6c9))
* target Docker Hub namespace and exact repository names ([412b6e0](https://github.com/magnus919/SlopSearX/commit/412b6e0deca832b9e2f2bd21bb136dbcf2edf4cb))
* use Oyez relevance search for named cases ([06e7658](https://github.com/magnus919/SlopSearX/commit/06e7658afe1cd023785dfbca897faa2675852a74))
* use phrase queries for arXiv relevance ([d09310b](https://github.com/magnus919/SlopSearX/commit/d09310b7298f008f9de1790085d6b44c3c875e33))
* use stateless JSON transport for FastMCP HTTP ([7bce9dd](https://github.com/magnus919/SlopSearX/commit/7bce9dd246f141eb961c52ae96061203a98f083b))
* validate Open Library ISBN shapes ([33efa04](https://github.com/magnus919/SlopSearX/commit/33efa04ed9c6ac5450a42e7b5e78ffe7751a32df))


### Documentation

* calibrate Jev specialist threshold value ([#398](https://github.com/magnus919/SlopSearX/issues/398)) ([d4e24ae](https://github.com/magnus919/SlopSearX/commit/d4e24ae322677fedbd8993ab33ae58da951070ff))
* clarify EXP-003 reporting scope and merged status [skip ci] ([#411](https://github.com/magnus919/SlopSearX/issues/411)) ([b4ee77b](https://github.com/magnus919/SlopSearX/commit/b4ee77b73f761d8803172c48a1531bcceef8bbcd))
* clarify production Jev opt-in across Compose profile ([daeeaf9](https://github.com/magnus919/SlopSearX/commit/daeeaf9183d8825d88f9cb51085e3350b27a8330))
* document Semantic Scholar API expectations ([3405b62](https://github.com/magnus919/SlopSearX/commit/3405b62182a7a34719b5f1a0522ba6e313766877))
* establish measured improvement loop [skip ci] ([d0aed40](https://github.com/magnus919/SlopSearX/commit/d0aed401298a84f41fed613fc84a8787e858b29a))
* establish measured improvement loop [skip ci] ([ecd261b](https://github.com/magnus919/SlopSearX/commit/ecd261bad209afe28c2aa0c243829b4587b63e19))
* evaluate Jev specialist augmentation ([#397](https://github.com/magnus919/SlopSearX/issues/397)) ([b8f5e53](https://github.com/magnus919/SlopSearX/commit/b8f5e535d29e1bcefcb6857f61797df464b9d607))
* freeze acquired cards and blinded Jev gold labels [skip ci] ([7bf802b](https://github.com/magnus919/SlopSearX/commit/7bf802bb419b08da1ae66e6833267cebfaabbfb0))
* freeze bounded Brave acquisition harness [skip ci] ([ff107ab](https://github.com/magnus919/SlopSearX/commit/ff107ab9c56035a162f69ecf4ef01c5d01095f95))
* freeze bounded npm candidate before Jev acquisition [skip ci] ([b624f6b](https://github.com/magnus919/SlopSearX/commit/b624f6bfbf782d3ff0440dd228649cbcbce19037))
* freeze composed Jev replay harness [skip ci] ([5e6834f](https://github.com/magnus919/SlopSearX/commit/5e6834fa0507fbf0e0dbbc6b6592a12290a3cab1))
* freeze Jev advisory evaluation and calibration harness [skip ci] ([9a044c9](https://github.com/magnus919/SlopSearX/commit/9a044c9646e4ff451f24c23bb2ca81bdd85af1ab))
* freeze Jev end-to-end acquisition harness [skip ci] ([c842c7b](https://github.com/magnus919/SlopSearX/commit/c842c7b811cd238779241f143a8fe306629f284a))
* freeze Jev utility-composition harness [skip ci] ([0c84e1e](https://github.com/magnus919/SlopSearX/commit/0c84e1ebf86a6e455e916d9cf2f7cc79ccfca69b))
* Jev advisory end-to-end research results ([1a84802](https://github.com/magnus919/SlopSearX/commit/1a848023797b97aabaf9d57f8cd2c3c9755c624e))
* Jev full-catalog composition experiment ([7d6bd06](https://github.com/magnus919/SlopSearX/commit/7d6bd06e8919fc54375fd7f733ebde53df9c87f0))
* link EXP-003 implementation and validation [skip ci] ([#393](https://github.com/magnus919/SlopSearX/issues/393)) ([d4fc9b7](https://github.com/magnus919/SlopSearX/commit/d4fc9b741795de55216ad49b1ac9d7c8a39d11e0))
* link experiment persistence PR [skip ci] ([b43ed00](https://github.com/magnus919/SlopSearX/commit/b43ed00b0e5cdb8315d09e98649049d35c44313d))
* preregister results-only MCP experiment [skip ci] ([fdbb8d1](https://github.com/magnus919/SlopSearX/commit/fdbb8d15d846f82c7a54dbdf8820bb0d7e914851))
* record bounded Jev experiment suite ([#396](https://github.com/magnus919/SlopSearX/issues/396)) ([6f53aaa](https://github.com/magnus919/SlopSearX/commit/6f53aaa0fc6e5f5927c16ceb727866c25491d19e))
* record calibrated Jev advisory experiment ([ae5cd4c](https://github.com/magnus919/SlopSearX/commit/ae5cd4c3cc86d83f9bd6963e961274be47e56ea9))
* record calibrated Jev advisory experiment [skip ci] ([642e4e4](https://github.com/magnus919/SlopSearX/commit/642e4e4fd8a18886d2d027a010c448eed08cf2fe))
* record completed EXP-001 retry and rejection [skip ci] ([331b9fa](https://github.com/magnus919/SlopSearX/commit/331b9fa0e66385743e9a4a0e688e2bead44d6a95))
* record composed Jev routing experiment ([b227a64](https://github.com/magnus919/SlopSearX/commit/b227a64f4fc82badd0653466fd87e9646b1fca00))
* record composed Jev routing replay [skip ci] ([9044cd2](https://github.com/magnus919/SlopSearX/commit/9044cd2ee64de93c733791dc5b3bad46c9338fc4))
* record EXP-001 MCP presentation experiment [skip ci] ([0424957](https://github.com/magnus919/SlopSearX/commit/042495724843b2b4d176084090d1c8068e94de62))
* record EXP-002 normalization experiment [skip ci] ([#388](https://github.com/magnus919/SlopSearX/issues/388)) ([ee2d663](https://github.com/magnus919/SlopSearX/commit/ee2d66332de21773e780934745f411383d7d3cf8))
* record EXP-003 language-report experiment [skip ci] ([#390](https://github.com/magnus919/SlopSearX/issues/390)) ([f944b94](https://github.com/magnus919/SlopSearX/commit/f944b94186f4c44fdcda5e29c463ed464d06ce77))
* record Jev advisory end-to-end readout [skip ci] ([a667109](https://github.com/magnus919/SlopSearX/commit/a6671099adaaca26ef591292cd7264829474d7c2))
* record Jev research spike [skip ci] ([d3a84fc](https://github.com/magnus919/SlopSearX/commit/d3a84fc25e3669bdd1fbd90ce3752138339ab8c1))
* record Jev utility-composition result [skip ci] ([11ad482](https://github.com/magnus919/SlopSearX/commit/11ad4823e5b5ce1622dcfe8e2c1e7f37c4467ec9))
* record TypeSafe Jev research spike ([1958abf](https://github.com/magnus919/SlopSearX/commit/1958abf711cbdb39328decfc8c2d04faa283c364))
* register composed Jev routing replay [skip ci] ([78b73f9](https://github.com/magnus919/SlopSearX/commit/78b73f90c7deed8c12d6abe652e7ba95b8685d52))
* register end-to-end Jev advisory experiments [skip ci] ([639d9e1](https://github.com/magnus919/SlopSearX/commit/639d9e1eb11ef5a2816133a926b00b129d8cef3d))
* register isolated EXP-001 retry [skip ci] ([5f17a55](https://github.com/magnus919/SlopSearX/commit/5f17a558475899f40558f943aa8bd956dd20fb5b))
* register Jev advisory calibration experiment [skip ci] ([3910a96](https://github.com/magnus919/SlopSearX/commit/3910a96138da7a814baea51790773a2e4487f461))
* register Jev reranking experiment [skip ci] ([3fe1993](https://github.com/magnus919/SlopSearX/commit/3fe19931505fb274546e6686711b3765a92d96b8))
* register Jev utility-composition experiment [skip ci] ([dd350c3](https://github.com/magnus919/SlopSearX/commit/dd350c3d9eef859c438e58be0aabb1a04bb900f4))
* retain aborted Jev trial and register bounded retry [skip ci] ([473f145](https://github.com/magnus919/SlopSearX/commit/473f145b16f9616e8ff8b68cdef3434ca1ac04b7))
* retain blocked free-engine trial and register bounded Brave retry [skip ci] ([aefe956](https://github.com/magnus919/SlopSearX/commit/aefe956a1fe4884fbf813fbeace8739153421f31))
* retain blocked MCP presentation experiment [skip ci] ([9709724](https://github.com/magnus919/SlopSearX/commit/9709724639a2ec0a46040793a6d79b2046b9ba67))

## [0.5.0](https://github.com/magnus919/SlopSearX/compare/v0.4.0...v0.5.0) (2026-09-11)


### Features

* add authenticated workflow console ([#370](https://github.com/magnus919/SlopSearX/issues/370)) ([086ea41](https://github.com/magnus919/SlopSearX/commit/086ea4189c89c57637263aba4f7822436cb6bfad))
* add bounded staged search workflow ([#344](https://github.com/magnus919/SlopSearX/issues/344)) ([65e9547](https://github.com/magnus919/SlopSearX/commit/65e9547475b9ee572b33ffc2b404c736a1b98e42))
* add caller-directed adaptive research continuations ([#334](https://github.com/magnus919/SlopSearX/issues/334)) ([9a28979](https://github.com/magnus919/SlopSearX/commit/9a28979e86651e6d548a397a5cc2caa8e99ec838))
* add durable workflow metrics ([#363](https://github.com/magnus919/SlopSearX/issues/363)) ([c6203b0](https://github.com/magnus919/SlopSearX/commit/c6203b027c592515a4c43f762afab78789b6c788))
* add human search portal foundation ([#333](https://github.com/magnus919/SlopSearX/issues/333)) ([56ea21b](https://github.com/magnus919/SlopSearX/commit/56ea21b982e49111dff13b362b73ad6464c5e1d3))
* add opt-in snapshot entity groups ([#314](https://github.com/magnus919/SlopSearX/issues/314)) ([e2df0b5](https://github.com/magnus919/SlopSearX/commit/e2df0b581109dd8ecdac9236cca97472682183e9))
* add retrieval receipts and research manifests ([#341](https://github.com/magnus919/SlopSearX/issues/341)) ([006c7c4](https://github.com/magnus919/SlopSearX/commit/006c7c4b7f4aa130318e173f038c41dac42f050d))
* add saved-search event outbox ([#369](https://github.com/magnus919/SlopSearX/issues/369)) ([d645fd9](https://github.com/magnus919/SlopSearX/commit/d645fd9d6f1c6346ec30c63149095d1da1e079f3))
* add scheduled saved-search change reports ([#339](https://github.com/magnus919/SlopSearX/issues/339)) ([16aa80e](https://github.com/magnus919/SlopSearX/commit/16aa80ea02277837b6e7675109efc5ea23ce16b7))
* add SearXNG format negotiation and 400 validation ([#302](https://github.com/magnus919/SlopSearX/issues/302)) ([ea95e23](https://github.com/magnus919/SlopSearX/commit/ea95e23913a37490d8a64b82a3ad0bda175ce2e9))
* add shared artifact lineage contracts ([#364](https://github.com/magnus919/SlopSearX/issues/364)) ([661ecc8](https://github.com/magnus919/SlopSearX/commit/661ecc88c5a138f40e08689b89c35a8bc6a3a112))
* add source-linked dependency dossiers ([#345](https://github.com/magnus919/SlopSearX/issues/345)) ([2b5469a](https://github.com/magnus919/SlopSearX/commit/2b5469aac14c06d3fc6368f45cbcdd681be2d338))
* complete portal search experience ([e9d530f](https://github.com/magnus919/SlopSearX/commit/e9d530f050d2560d44350071a2054abda5d70abd))
* compose workflows from artifacts ([#366](https://github.com/magnus919/SlopSearX/issues/366)) ([6c5d3a4](https://github.com/magnus919/SlopSearX/commit/6c5d3a49fc4b1e5d3c0e3146afd3408168d032a2))
* enrich portal search result presentation ([a5b32c2](https://github.com/magnus919/SlopSearX/commit/a5b32c2c6fa10138e4d774f257a06ec5d041ae58))
* explain portal search results ([#367](https://github.com/magnus919/SlopSearX/issues/367)) ([9b2fb68](https://github.com/magnus919/SlopSearX/commit/9b2fb680da416c092cfbcd35cd95e8b4c5518475))
* extend entity grouping identities ([#368](https://github.com/magnus919/SlopSearX/issues/368)) ([6267dab](https://github.com/magnus919/SlopSearX/commit/6267dab02a9dfab00ccf6fb93dc09c09b4693d6d))
* harden portal policy and contribution gates ([#340](https://github.com/magnus919/SlopSearX/issues/340)) ([bd03748](https://github.com/magnus919/SlopSearX/commit/bd03748ee32003f88d61c4704b92f3a8823a8a5b))
* support SearXNG routes and healthz ([#301](https://github.com/magnus919/SlopSearX/issues/301)) ([db09277](https://github.com/magnus919/SlopSearX/commit/db09277600fce0509c219234a84db4687f7ab082))


### Bug Fixes

* align SearXNG config and parsed URLs ([#299](https://github.com/magnus919/SlopSearX/issues/299)) ([987e936](https://github.com/magnus919/SlopSearX/commit/987e936407bd430bb18c2f1c993faa6bca602181))
* **ci:** route Droid reviews through gpuslut ([#336](https://github.com/magnus919/SlopSearX/issues/336)) ([9a1a01b](https://github.com/magnus919/SlopSearX/commit/9a1a01b2c75da50e3098fb893dc404a68ea22d35))
* **ci:** route Droid wiki through gpuslut ([#337](https://github.com/magnus919/SlopSearX/issues/337)) ([9d0f594](https://github.com/magnus919/SlopSearX/commit/9d0f594bf4297cf04005b069cecd34ee6510010e))
* clarify portal navigation ([ccad0d5](https://github.com/magnus919/SlopSearX/commit/ccad0d5950a596ad5986ab15b1d5c8bdd12cb16d))
* improve portal trust and scanability ([1d8daaa](https://github.com/magnus919/SlopSearX/commit/1d8daaaf2b8cf5aa436b757519bb36fbf02a107d))
* make portal pagination honest ([171532a](https://github.com/magnus919/SlopSearX/commit/171532a5c8bfbb7cdafdde82e5bbf460e30c3d44))
* remove build-only pip from runtime image ([9e2574e](https://github.com/magnus919/SlopSearX/commit/9e2574e8934aa3c0d86b4e81c85604aa91849c7d))


### Documentation

* define browser identity and tenant isolation ([#361](https://github.com/magnus919/SlopSearX/issues/361)) ([6399772](https://github.com/magnus919/SlopSearX/commit/6399772a558a1e3b7cbaed6d787b8d9b13482b71))
* establish portal decisions and release gates ([53cdadf](https://github.com/magnus919/SlopSearX/commit/53cdadf044dac6f6655ff0e9d6ab9236ecde0dbd))
* evaluate retrieval quality metadata ([#365](https://github.com/magnus919/SlopSearX/issues/365)) ([f4c7f96](https://github.com/magnus919/SlopSearX/commit/f4c7f960f09a1b2ce931d1e0bd2bf1a171cb5222))
* package and verify portal release ([#342](https://github.com/magnus919/SlopSearX/issues/342)) ([5622a90](https://github.com/magnus919/SlopSearX/commit/5622a902563f0d6206aac282dddb283704d8b28d))

## [0.4.0](https://github.com/magnus919/SlopSearX/compare/v0.3.1...v0.4.0) (2026-09-07)


### Features

* add opt-in interactive search deadlines ([#268](https://github.com/magnus919/SlopSearX/issues/268)) ([f20a6bc](https://github.com/magnus919/SlopSearX/commit/f20a6bc5f14f2574ac8cda2a42d025411f7ff1aa))
* add opt-in reciprocal rank fusion with judged evaluation ([#249](https://github.com/magnus919/SlopSearX/issues/249)) ([e72e05f](https://github.com/magnus919/SlopSearX/commit/e72e05f11a207887a662789bf8ff110af8415851))
* enforce audited OpenAlex publication date constraints ([#270](https://github.com/magnus919/SlopSearX/issues/270)) ([37cdc23](https://github.com/magnus919/SlopSearX/commit/37cdc2357dce2f46df74d016c782b7aae3b53453))


### Bug Fixes

* bound degraded search cache lifetimes ([#266](https://github.com/magnus919/SlopSearX/issues/266)) ([7146bf8](https://github.com/magnus919/SlopSearX/commit/7146bf81854a9a58979c59baccf659500ad190ac))
* bound metrics storage and validate monitoring contracts ([#267](https://github.com/magnus919/SlopSearX/issues/267)) ([70aad1e](https://github.com/magnus919/SlopSearX/commit/70aad1ed7be4519ad6c2734b25dcb2b588e34f1a))
* correct OpenAlex links timing and relevance ([#260](https://github.com/magnus919/SlopSearX/issues/260)) ([1fc5ced](https://github.com/magnus919/SlopSearX/commit/1fc5cedd7123a7d1138e2a3e45f6a93f129147c3))
* honor HTTP response format on cache hits ([#259](https://github.com/magnus919/SlopSearX/issues/259)) ([0e4ff88](https://github.com/magnus919/SlopSearX/commit/0e4ff88c039a2057151f519919f989ae53771895))
* preserve cache identity and coalesce concurrent searches ([#252](https://github.com/magnus919/SlopSearX/issues/252)) ([a4df993](https://github.com/magnus919/SlopSearX/commit/a4df993f38d8d2fa7bde8c8661265823cdd98e0d))
* retain actual ranking provenance in cached results and snapshots ([#255](https://github.com/magnus919/SlopSearX/issues/255)) ([3a693d6](https://github.com/magnus919/SlopSearX/commit/3a693d6a1e01f4882977171aa2c9a52f336d6b7a))


### Performance Improvements

* reuse bounded adapter HTTP connection pools ([#250](https://github.com/magnus919/SlopSearX/issues/250)) ([eee52d2](https://github.com/magnus919/SlopSearX/commit/eee52d23c5cdf893333faf52ed7ad16480daa5a2))
* schedule research jobs through durable ready indexes ([#253](https://github.com/magnus919/SlopSearX/issues/253)) ([955ba74](https://github.com/magnus919/SlopSearX/commit/955ba74bc8fa00e3d96eb0b2667900d584f6988f))

## [0.3.1](https://github.com/magnus919/SlopSearX/compare/v0.3.0...v0.3.1) (2026-08-26)


### Bug Fixes

* **deps:** restore pydantic-core 2.46.4 for lockfile coherence with pydantic 2.13.4 ([d17a847](https://github.com/magnus919/SlopSearX/commit/d17a84715571d0dfbd995a864ae45ad2082507b3))
* **duckduckgo:** add lite fallback, realistic session, honest block classification ([e68448f](https://github.com/magnus919/SlopSearX/commit/e68448f8c851b081da117840f92089fdef8090a8))
* **duckduckgo:** add lite fallback, realistic session, honest block classification ([a3b1c3a](https://github.com/magnus919/SlopSearX/commit/a3b1c3abf956ed75f447be921dd27a47d11b3601))
* **duckduckgo:** per-origin Sec-Fetch headers, bounded bootstrap, honest fallback detail ([b6c142e](https://github.com/magnus919/SlopSearX/commit/b6c142e310119bb390fb2f8e3a2d4170cf57dce3))
* guard image digest pin lifecycle per review findings ([bd4bb85](https://github.com/magnus919/SlopSearX/commit/bd4bb85d97d750ae8794b54e9d4c0f506be53f6e))
* pin compose and k8s deployments to CI-published GHCR digest ([28c4ace](https://github.com/magnus919/SlopSearX/commit/28c4ace0f675006edf1e0805b1594428f96fee6a))
* pin compose and k8s deployments to CI-published GHCR digest ([372328f](https://github.com/magnus919/SlopSearX/commit/372328f5c39580d1fad86fe11ad251cf539b9eb3))
* **test:** drop hardcoded version literal from VAL-DIAG-002 ([426df20](https://github.com/magnus919/SlopSearX/commit/426df20d10a21973f87045dc52446fe610f611b6))
* **test:** drop hardcoded version literal from VAL-DIAG-002 ([eea5ba3](https://github.com/magnus919/SlopSearX/commit/eea5ba35a867269cad5f3a4920c13354e948d6fd))
* **test:** drop hardcoded version literal from VAL-DIAG-002 ([#213](https://github.com/magnus919/SlopSearX/issues/213)) ([426df20](https://github.com/magnus919/SlopSearX/commit/426df20d10a21973f87045dc52446fe610f611b6))


### Documentation

* add MCP full-strength access PRD ([fb60312](https://github.com/magnus919/SlopSearX/commit/fb60312c91657f2c986c7449ababa9d5e4780542))
* add MCP full-strength access PRD ([#212](https://github.com/magnus919/SlopSearX/issues/212)) ([fb60312](https://github.com/magnus919/SlopSearX/commit/fb60312c91657f2c986c7449ababa9d5e4780542))
* drop stale Unreleased changelog section (content shipped in v0.3.0) ([2d97c97](https://github.com/magnus919/SlopSearX/commit/2d97c970fe06217fe6a5d9181e92b851785b4165))

## [0.3.0](https://github.com/magnus919/SlopSearX/compare/v0.2.0...v0.3.0) (2026-08-22)


### Features

* add curated non-secret operational diagnostics to status and health ([28c39f3](https://github.com/magnus919/SlopSearX/commit/28c39f3d7d3da6c068677afab6a2824235f060b0))
* add deterministic MCP fixture harness over streamable HTTP ([def7497](https://github.com/magnus919/SlopSearX/commit/def749765d53b6bd421d3c3c0925fee16a912e77))
* add explainable cost- and coverage-aware routing ([b8103fc](https://github.com/magnus919/SlopSearX/commit/b8103fc3406afc5025c058d62ce449d3fbed13ab))
* add explainable cost- and coverage-aware routing ([d11d105](https://github.com/magnus919/SlopSearX/commit/d11d105e193fe876eb757616bb9e0b6a57a5e89f))
* add explicit image/video media search and result contract ([2dceac1](https://github.com/magnus919/SlopSearX/commit/2dceac137867e50dbc7c9be7f8c6cbd70bd548be))
* add explicit media search and result contract ([85e710b](https://github.com/magnus919/SlopSearX/commit/85e710bc4775a9005aca4714ec86bc1fe5a6bbf9))
* add jobs search topic and ATS engine adapters (Greenhouse, Ashby, Lever) ([#132](https://github.com/magnus919/SlopSearX/issues/132)) ([dfd517c](https://github.com/magnus919/SlopSearX/commit/dfd517c545a1d84c2a8f4be04a651694e6c18cc1))
* add MCP server for AI agents ([af763ab](https://github.com/magnus919/SlopSearX/commit/af763ab9c6a35f8635a48ea47e0f49fac3e00a4c))
* add MCP server for AI agents ([c1ae16f](https://github.com/magnus919/SlopSearX/commit/c1ae16f21c3be6013b2bd8987c836113aac07c27))
* add selective research retry and bounded follow-up ([2d5e8c6](https://github.com/magnus919/SlopSearX/commit/2d5e8c6977cdcb9573c8619f9b247953d4e42baf))
* add structured filter-enforcement report to MCP search tools ([9797ac7](https://github.com/magnus919/SlopSearX/commit/9797ac7c9280888088fb4967697f5dea7e3cdea1))
* audit and declare engine capability metadata ([2e32f3c](https://github.com/magnus919/SlopSearX/commit/2e32f3c27407c27ca7aae8ff91c6de75abae7fbe))
* audit and declare engine capability metadata ([18e9736](https://github.com/magnus919/SlopSearX/commit/18e97367c4fe0a9ab0dc467acb10e9d7545be5cb))
* derive live engine health from observed outcomes ([0fd7bde](https://github.com/magnus919/SlopSearX/commit/0fd7bde997303a323eb6bca044dd662e755d61b8))
* derive live engine health from observed outcomes ([4ff5000](https://github.com/magnus919/SlopSearX/commit/4ff5000080fcc483f0af4687d64bc8d17da42c5e))
* diagnose empty scrape responses ([51ee882](https://github.com/magnus919/SlopSearX/commit/51ee882c96f220a404aa87f11df12459ecf87086))
* diagnose empty scrape responses ([6004d81](https://github.com/magnus919/SlopSearX/commit/6004d8149c9cbdf845649a48f0283048071cc998))
* enforce and report search filter semantics ([f3ae26a](https://github.com/magnus919/SlopSearX/commit/f3ae26a712bf1454dc05f60f2bab1567dd1eb03e))
* enforce and report search filter semantics ([b26d1f3](https://github.com/magnus919/SlopSearX/commit/b26d1f38c4aa3e22613e11f9ecca459bc29e7c84))
* expose full engine capability matrix from the live catalog ([6fe1c5c](https://github.com/magnus919/SlopSearX/commit/6fe1c5c2a4e8d1646124ac87207ec178a9bd9c5e))
* expose machine-readable search-to-retrieval handoff boundary ([c1c510a](https://github.com/magnus919/SlopSearX/commit/c1c510a862f67761fc1bd97f114bdcd2e4ecb4ec))
* full-strength MCP access for SlopSearX ([f922728](https://github.com/magnus919/SlopSearX/commit/f9227280e6c2a3ef95364a954e9b75a5eb25a5fd))
* harden snapshot/cursor lifecycle with expiry and unavailability semantics ([87e2781](https://github.com/magnus919/SlopSearX/commit/87e27819e79c0dbd321cea03f811f150c9a26d46))
* implement progressive-disclosure result card/record contract ([c788662](https://github.com/magnus919/SlopSearX/commit/c788662027bc720f65cfa72ca36dddfa415b4a7e))
* make research job execution durable across replicas ([7c9993e](https://github.com/magnus919/SlopSearX/commit/7c9993e50e0a03fe571479712bd4874386ac19de))
* make research job execution durable across replicas ([0b370be](https://github.com/magnus919/SlopSearX/commit/0b370be0821cbe6de0a794257be12e778228e698))
* persist research per-query and per-engine coverage with failure classes ([5365ba1](https://github.com/magnus919/SlopSearX/commit/5365ba180a3da6e4113853015002043c8723a5b8))
* preserve typed domain payloads in normalized results ([9b020c8](https://github.com/magnus919/SlopSearX/commit/9b020c88380736a56a198fd7ae3880a4189eb234))
* preserve typed domain payloads in normalized results ([6c8f4ef](https://github.com/magnus919/SlopSearX/commit/6c8f4ef2fa51bd2297b4b475c881b811f5e33ca8))
* raise Brave default result cap ([#207](https://github.com/magnus919/SlopSearX/issues/207)) ([03b294f](https://github.com/magnus919/SlopSearX/commit/03b294f0a7a4b7652e1f6430f0d5cf3722ad2874))
* recover discarded evidence in the MCP search envelope ([192b8a9](https://github.com/magnus919/SlopSearX/commit/192b8a916594703eee55f489477a7986fab7e9fa))
* unify sensitive-engine and specialist-grant policy gate ([61cc3f3](https://github.com/magnus919/SlopSearX/commit/61cc3f3b572a9209232e2061fae254f7691f8065))


### Bug Fixes

* add type args to bare dict in __init__ for mypy 2.2.0 ([9bc5b1a](https://github.com/magnus919/SlopSearX/commit/9bc5b1a684a9f53c8d48cc9e808ddbd16488066f))
* align payload/media disclosure gates with persistence boundary ([cb423d4](https://github.com/magnus919/SlopSearX/commit/cb423d476dec514fde915f702eabc8381cc94a86))
* align payload/media disclosure gates with persistence boundary ([ef7b065](https://github.com/magnus919/SlopSearX/commit/ef7b065e0be6e93bf6992e8b3eb0989f738a2d45))
* apply base-image security updates ([#209](https://github.com/magnus919/SlopSearX/issues/209)) ([851090f](https://github.com/magnus919/SlopSearX/commit/851090f246802097bc6f460923351c325bd688f3))
* apply persistence bound to HTTP payload output gate ([e57963d](https://github.com/magnus919/SlopSearX/commit/e57963da9e29a844018f4e8829c8325cf4f32b37))
* bound the aggregate dispatch deadline ([0f78d0d](https://github.com/magnus919/SlopSearX/commit/0f78d0da3ebfb64a78f306312b022eb2244d6168))
* **brave:** load API key from environment variable as fallback ([9096879](https://github.com/magnus919/SlopSearX/commit/90968797d73953fb448a7b9506d70e4cca228e3f))
* **brave:** load API key from environment variable as fallback ([9096879](https://github.com/magnus919/SlopSearX/commit/90968797d73953fb448a7b9506d70e4cca228e3f))
* **brave:** load API key from environment variable as fallback ([8f7f8b7](https://github.com/magnus919/SlopSearX/commit/8f7f8b77837f0b44581ace9107aa40053a2a2efc))
* **brave:** load API key in __init__ so health check passes at startup ([9becad0](https://github.com/magnus919/SlopSearX/commit/9becad0d7b9ee954c76d7c90e514278fe4f1e416))
* **brave:** load API key in __init__ so health check passes at startup ([9becad0](https://github.com/magnus919/SlopSearX/commit/9becad0d7b9ee954c76d7c90e514278fe4f1e416))
* **brave:** load API key in __init__ so health check passes at startup ([f833807](https://github.com/magnus919/SlopSearX/commit/f8338079db3b8abec1d94cbe483a5f5d247d5b11))
* cache canonical full response and derive per-request view ([9c64dbc](https://github.com/magnus919/SlopSearX/commit/9c64dbcb96dcf00749a500959d807a8d34652caf))
* centralize feature env overrides ([6031f62](https://github.com/magnus919/SlopSearX/commit/6031f625a97283acccd9113066b90a7a3535de5b))
* centralize feature env overrides ([bebdaa3](https://github.com/magnus919/SlopSearX/commit/bebdaa34043989e9e3121285e37e28e8f2169c78))
* **ci:** skip droid-review for dependabot PRs ([a14910b](https://github.com/magnus919/SlopSearX/commit/a14910b33bdac1300cbae957ff326f1c5e5dfa6f))
* clear stale research leases, guard saves, use SCAN ([a2a697d](https://github.com/magnus919/SlopSearX/commit/a2a697d8af35019062f9c9bbf360bd4157c26a65))
* close media routing gaps from issue-188 code review ([e088544](https://github.com/magnus919/SlopSearX/commit/e088544c992a77563fe1a1d2149f91032bd2e20e))
* close SSRF gaps in retrieval URL handoff guard ([3112c20](https://github.com/magnus919/SlopSearX/commit/3112c20c098404d7553cf4663a453fce12d08ed8))
* complete timeout status follow-up contract ([f11e15f](https://github.com/magnus919/SlopSearX/commit/f11e15fd8c937c19545943e6eb07deb884fda761))
* expire deadline-passed claims and guard direct runs ([fc2b789](https://github.com/magnus919/SlopSearX/commit/fc2b789e9cd229294e5846a44ec543496da348bf))
* finalize lapsed-deadline runs before mutation, scan colon-safe tenants ([40afa0c](https://github.com/magnus919/SlopSearX/commit/40afa0cee77de02e422974b7b12b587fb9f0b57a))
* fold observed health into routing cache digest ([dc19d08](https://github.com/magnus919/SlopSearX/commit/dc19d088531e2f0a45581cc6374417d7962048a2))
* fold sensitive/tier1 sets into routing digest; freeze HTTP budget ([f15a73b](https://github.com/magnus919/SlopSearX/commit/f15a73b06d8978634ed114381c7c7f7332e1a7e8))
* fold sensitive/tier1 sets into routing digest; freeze HTTP budget ([f3e084f](https://github.com/magnus919/SlopSearX/commit/f3e084f686167402060688f36981cbe6875e61fc))
* gate cancel/retry/extend and keep research leases alive ([78ac88a](https://github.com/magnus919/SlopSearX/commit/78ac88a6fe2bcc7eb92d9db6b42f8e57a83b59b1))
* gate health folding in routing digest on engine-count cap ([c8739ac](https://github.com/magnus919/SlopSearX/commit/c8739acaba5597f3ebb265a3c0ccd61c5e14c0d7))
* gate payload serialization the renderer way and cap requested inline ([a298d5a](https://github.com/magnus919/SlopSearX/commit/a298d5acbc6bab03381e59c963f55930325c9d4b))
* handle arXiv HTTPS redirects safely ([8985874](https://github.com/magnus919/SlopSearX/commit/8985874718c0afe50e9e2233bd32ff10f913f4d7))
* harden payload disclosure and CVSS parity ([0957a1e](https://github.com/magnus919/SlopSearX/commit/0957a1ecffc362c9edeac40071bd0ae42d834fce))
* harden payload serialization, persistence bound, and CVSS parity ([f780274](https://github.com/magnus919/SlopSearX/commit/f7802747e99982e56f7ce275efea7cf512f11fbe))
* harden retrieval handoff URL classification and docs ([45fd16c](https://github.com/magnus919/SlopSearX/commit/45fd16c1c7ca6165074ae35c7a2d94f80391d0c9))
* harden snapshot TTL and research retry/extend edge cases ([e3a04f9](https://github.com/magnus919/SlopSearX/commit/e3a04f9737ddb5b8720d11b3dffdb9f2fe536db1))
* harden URL handoff guard and dedup against malformed hosts ([4c99744](https://github.com/magnus919/SlopSearX/commit/4c9974431d75e26ead8c47e3dc857de1bacb9896))
* honor per-engine timeout_ms at search dispatch ([#184](https://github.com/magnus919/SlopSearX/issues/184)) ([23dc38c](https://github.com/magnus919/SlopSearX/commit/23dc38c92b217c78c70c4c4b7f02bf0feb9b2a36))
* install cssselect at runtime ([2433336](https://github.com/magnus919/SlopSearX/commit/243333698ce2f0f7d7550f090af262f35e5afc5e))
* install cssselect at runtime ([e859bc2](https://github.com/magnus919/SlopSearX/commit/e859bc273e7e30089c9b1e73b60ee0f08441cd4c))
* keep cached scope live and pin routing test config ([d5cd05a](https://github.com/magnus919/SlopSearX/commit/d5cd05ac90a944551ab0ace391c481b81dc9dfa6))
* keep intent media type across explicit scope in _resolve_scope ([f97f578](https://github.com/magnus919/SlopSearX/commit/f97f578300eb47d9267c4b2cc41059416ecd3bb7))
* lease direct research runs and check lease liveness ([3999081](https://github.com/magnus919/SlopSearX/commit/3999081f9eb0547eb03ca382f47ae06e1cb25ed9))
* make filter enforcement value-aware and honest in warnings ([0924989](https://github.com/magnus919/SlopSearX/commit/0924989f9056632db2ae6d595ebf7f0c9652e0ab))
* make research lease primitives atomic and claim terminal retries ([f1da60c](https://github.com/magnus919/SlopSearX/commit/f1da60c6cf08efb259858e4e6456be285de5e775))
* make SearchResult serialization JSON-safe ([6b7447a](https://github.com/magnus919/SlopSearX/commit/6b7447a35be5657eb610512c1f3c552c22ef7d4f))
* memoize /health config+catalog and never fabricate observed latency ([8d77e38](https://github.com/magnus919/SlopSearX/commit/8d77e3830f7e60cc24596b8685cec67ad6a7e019))
* never fabricate observed health latency or auth state ([0005392](https://github.com/magnus919/SlopSearX/commit/00053923eef11a8782873bb83e42cda22e0425b1))
* never hand off SSRF-prone IP hosts or userinfo URLs ([9f1730c](https://github.com/magnus919/SlopSearX/commit/9f1730c750b3a3ae4404c6d9dcc40eee4e47a71d))
* only count the local layer for filters with a local post-filter ([b44b20b](https://github.com/magnus919/SlopSearX/commit/b44b20b3acee3f40d7b3d4f10788387428c42472))
* pin Docker image provenance ([#211](https://github.com/magnus919/SlopSearX/issues/211)) ([77d1448](https://github.com/magnus919/SlopSearX/commit/77d1448727c35c830af04acde0893d20fc160ef2))
* pin sanitized error-message assertion to exact output ([32c47fd](https://github.com/magnus919/SlopSearX/commit/32c47fda739af1213e795f7cf689b3fcba58ad76))
* preserve arXiv rate-limit classification ([28d4670](https://github.com/magnus919/SlopSearX/commit/28d4670f729ccc97a9cf538467fb3e5b92e4923c))
* preserve configured timeout in fan-out deadline ([9ab5c37](https://github.com/magnus919/SlopSearX/commit/9ab5c37df02ffd98d3b855d092b4d12ab4c1ac29))
* **ratelimit:** remove unused sidecar stub ([9b22616](https://github.com/magnus919/SlopSearX/commit/9b22616ae6c718b7b64673ac0978354f6cab5084))
* **ratelimit:** remove unused sidecar stub ([9ae0dad](https://github.com/magnus919/SlopSearX/commit/9ae0dadaa05e4059f9b33533b625ec4ecce2b03d))
* reconcile direct runs and surface durable cancellation ([de1f754](https://github.com/magnus919/SlopSearX/commit/de1f75414db358e425f5ca61b4425205bec3f33d))
* reject IPv6 6to4 literals and distinguish reserved-prefix reasons ([635bc6c](https://github.com/magnus919/SlopSearX/commit/635bc6c5aa8d4fecaaa48dd093ea3a85f9a9f41a))
* reject IPv6 translation-prefix and site-local retrieval targets ([a18d889](https://github.com/magnus919/SlopSearX/commit/a18d88955330b2d721d29c5acd0d5a5285b8b61b))
* reject IPv6 translation-prefix and site-local retrieval targets ([427ade1](https://github.com/magnus919/SlopSearX/commit/427ade12ce3ad1f5f750efb4937ba4f293d05291))
* reject percent-encoded and non-ASCII hosts in retrieval handoff ([7c52d2a](https://github.com/magnus919/SlopSearX/commit/7c52d2a4217f60e55964c54200be7b14211fb860))
* remove dead payload catalog and gate read-result payload ([f495e58](https://github.com/magnus919/SlopSearX/commit/f495e586b840bc527a7e7abbaacb3d4c70b51324))
* route strict-safesearch preview through the real query scope ([373b52e](https://github.com/magnus919/SlopSearX/commit/373b52ead486e50f8d7af784cf0af032df2eb718))
* run routing pass on media path and pin harness config ([7e3c0df](https://github.com/magnus919/SlopSearX/commit/7e3c0dff883a73713035534fbb611a4d631198d6))
* satisfy both mypy gate versions for MCP SDK redirect helper ([de3d939](https://github.com/magnus919/SlopSearX/commit/de3d93979021933a836345c0646c49120cd8fa53))
* scope IPv6 reserved check so IPv4-mapped literals embed-safe ([776c431](https://github.com/magnus919/SlopSearX/commit/776c4318c5fd1d00ee610304f900df52f5d0412b))
* strict payload size gating and CVSS/FRED fixes ([64b2007](https://github.com/magnus919/SlopSearX/commit/64b200725450aaabaf96c2bb39d645c2eb45a14c))
* sync ctx sensitive set from policy; harden routing eval bounds ([965e1f3](https://github.com/magnus919/SlopSearX/commit/965e1f359ca39616c41e3fe0f70c68e7c0b32630))
* treat empty max_cost_class as permissive; fix R5 eval baseline ([f615341](https://github.com/magnus919/SlopSearX/commit/f615341229a40fc4b1ce3b8001bc54c23a3dfad4))


### Documentation

* add engine troubleshooting guidance ([f350d49](https://github.com/magnus919/SlopSearX/commit/f350d496fcf93bbf02cf2920849e8f713e06b2dc))
* add engine troubleshooting guidance ([59e796c](https://github.com/magnus919/SlopSearX/commit/59e796ca1262f829f51c666479c54c552f136af4))
* add field-level MCP contract mapping ([07186a7](https://github.com/magnus919/SlopSearX/commit/07186a7cc26eb2823c1c97a14f5c7b2bb836f30f))
* correct stale claims and document MCP invariants ([0f4f725](https://github.com/magnus919/SlopSearX/commit/0f4f725d7a5853a442b3aafa67063839c2045a5d))
* define the search-to-retrieval handoff boundary ([4dd4e8a](https://github.com/magnus919/SlopSearX/commit/4dd4e8a9495c75220a2f286c8bf53310c4251d5b))
* document advanced-search decision and finalize CI parity ([7da23a6](https://github.com/magnus919/SlopSearX/commit/7da23a6228cb63fa3c21f7f294308dd4a116bcf2))
* expose unavailable health status ([95d9a1b](https://github.com/magnus919/SlopSearX/commit/95d9a1b2f69e878cbb6c8fd3eab9e178e6e61ca9))

## [0.2.0](https://github.com/magnus919/SlopSearX/compare/v0.1.1...v0.2.0) (2026-07-02)


### Features

* add error tracking, alerting, product analytics, and error-to-insight pipeline ([77caf77](https://github.com/magnus919/SlopSearX/commit/77caf77c044bc61b3a5705368d386d5b66f66cbc))
* add error tracking, alerting, product analytics, and error-to-insight pipeline ([7a7eeea](https://github.com/magnus919/SlopSearX/commit/7a7eeea71a875dbd39f07e55ea8c9bcdc7dd3f5e))
* add feature flag infrastructure and regenerate wiki ([5864cdf](https://github.com/magnus919/SlopSearX/commit/5864cdf86c0c82c848a94d9f0be9e24f62a67fd4))
* add image search support to DuckDuckGo adapter ([#124](https://github.com/magnus919/SlopSearX/issues/124)) ([5bc615e](https://github.com/magnus919/SlopSearX/commit/5bc615e47b1e7eca9c1c370940c0b9d58f7a0a12))
* add pre-commit hooks, complexity, dead-code, duplicate detection, and import-linter ([b2ff3a3](https://github.com/magnus919/SlopSearX/commit/b2ff3a38c12b14dffa5a8fb11d0728c6714491e1))
* add pre-commit hooks, complexity/dead-code/duplicate detection, and import-linter ([8d26d55](https://github.com/magnus919/SlopSearX/commit/8d26d552d4fb284d39e6ff1acc275fffdc34a22f))
* aggressive caching, circuit breaker, query audit trail ([#92](https://github.com/magnus919/SlopSearX/issues/92)) ([#93](https://github.com/magnus919/SlopSearX/issues/93)) ([54bd5be](https://github.com/magnus919/SlopSearX/commit/54bd5becfa99c9924ea17a9f6efca35f52f5e9f6))
* fix 12 remaining Agent Readiness signals ([f4a230a](https://github.com/magnus919/SlopSearX/commit/f4a230acf2081762abc97c3618e157d516c0a27b))
* fix 12 remaining Agent Readiness signals ([615849d](https://github.com/magnus919/SlopSearX/commit/615849d70aa38073ad0d4200f225070bfb107715))


### Bug Fixes

* correct CI regressions from pipeline hardening ([ca3e52b](https://github.com/magnus919/SlopSearX/commit/ca3e52b542bfa32132f8906ff3d45d7f48864943))
* integration tests for CI without Valkey ([1e1e89d](https://github.com/magnus919/SlopSearX/commit/1e1e89d2d1955472d83e2e4eaf26a762bf39c249))
* route Brave adapter by category instead of always hitting /web ([13133e7](https://github.com/magnus919/SlopSearX/commit/13133e79b663a8ef7b2d392d7cd77948e96f3e5c)), closes [#123](https://github.com/magnus919/SlopSearX/issues/123)
* stop hitting Brave API in health checks ([393a746](https://github.com/magnus919/SlopSearX/commit/393a7464d9e5ecffa2f6457c661a2aed883cf50d))


### Documentation

* add feature flag workflow and pre-commit guidance ([1c7a2cf](https://github.com/magnus919/SlopSearX/commit/1c7a2cf7ff3005f8a34e06342651fb25e63e98d2))
