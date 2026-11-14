// Local design data; no service requests or live game control.
window.LunaDesignData = {
  "games": [
    {
      "id": "sakura",
      "title": "サクラノ詩 10th Anniversary edition",
      "subtitle": "サクラノ詩",
      "exe": "sakuranouta10th.exe",
      "arch": "x64",
      "bottle": "Sakura-no-Uta-10th",
      "accent": "#ae648c",
      "cover": "assets/sakura-01.png",
      "scene": "assets/sakura-01.png",
      "language": "ja",
      "hook": "Artemis",
      "streams": [
        {
          "id": "sakura-body",
          "label": "Artemis",
          "hook": "Artemis",
          "preview": "その風景には、悲しさも、痛みも、なかった。",
          "address": "5370399856",
          "context": "0",
          "context2": "0",
          "kind": "正文候选"
        },
        {
          "id": "sakura-secondary",
          "label": "Artemis",
          "hook": "Artemis",
          "preview": "セーブ　ロード　設定",
          "address": "5370399888",
          "context": "1",
          "context2": "0",
          "kind": "界面文本候选"
        }
      ],
      "lines": [
        {
          "id": "sakura-001",
          "gameId": "sakura",
          "source": "その風景には、悲しさも、痛みも、なかった。",
          "rawSource": "その風景には、悲しさも、痛みも、なかった。",
          "translation": "那片景色中，没有悲伤，也没有痛楚。",
          "tokens": [
            {
              "surface": "その",
              "reading": "",
              "lemma": "その",
              "pos": "连体词",
              "meaning": "那个；用于指代前文或双方已知的对象。"
            },
            {
              "surface": "風景",
              "reading": "ふうけい",
              "lemma": "風景",
              "pos": "名词",
              "meaning": "风景；景色。"
            },
            {
              "surface": "には",
              "reading": "",
              "lemma": "には",
              "pos": "助词",
              "meaning": "场所或对象「に」与主题「は」的组合，依语境理解。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "悲しさ",
              "reading": "かなしさ",
              "lemma": "悲しい",
              "pos": "名词",
              "meaning": "悲伤；悲伤的程度。由形容词「悲しい」加「さ」名词化。"
            },
            {
              "surface": "も",
              "reading": "",
              "lemma": "も",
              "pos": "助词",
              "meaning": "也；与否定呼应时表示都不。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "痛み",
              "reading": "いたみ",
              "lemma": "痛み",
              "pos": "名词",
              "meaning": "疼痛；痛楚。"
            },
            {
              "surface": "も",
              "reading": "",
              "lemma": "も",
              "pos": "助词",
              "meaning": "也；与否定呼应时表示都不。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "なかった",
              "reading": "",
              "lemma": "ない",
              "pos": "助动词",
              "meaning": "「ない」的过去形式；没有。"
            },
            {
              "surface": "。",
              "reading": "",
              "lemma": "。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/sakura-01.png",
          "pageGroup": 1,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sakura-002",
          "gameId": "sakura",
          "source": "だから、俺は泣かなかったのだろうか？",
          "rawSource": "だから、俺は泣かなかったのだろうか？",
          "translation": "所以，我才没有哭出来吗？",
          "tokens": [
            {
              "surface": "だから",
              "reading": "",
              "lemma": "だから",
              "pos": "接续词",
              "meaning": "因此；所以。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "俺",
              "reading": "おれ",
              "lemma": "俺",
              "pos": "代词",
              "meaning": "我。较随意的男性自称，语气依人物而定。"
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "泣かなかった",
              "reading": "なかなかった",
              "lemma": "泣く",
              "pos": "动词",
              "meaning": "没有哭。「泣く」的否定过去形。"
            },
            {
              "surface": "のだろうか",
              "reading": "",
              "lemma": "のだろうか",
              "pos": "表达",
              "meaning": "表示推测性的疑问或自问。"
            },
            {
              "surface": "？",
              "reading": "",
              "lemma": "？",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/sakura-02.png",
          "pageGroup": 2,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sakura-003",
          "gameId": "sakura",
          "source": "風に舞う薄紅は、たしかに美しくはあったが、俺にはただそれだけの事としか感じられない。",
          "rawSource": "風に舞う薄紅は、たしかに美しくはあったが、俺にはただそれだけの事としか感じられない。",
          "translation": "随风起舞的淡红花瓣确实很美，可对我来说，也仅此而已。",
          "tokens": [
            {
              "surface": "風",
              "reading": "かぜ",
              "lemma": "風",
              "pos": "名词",
              "meaning": "风。"
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "舞う",
              "reading": "まう",
              "lemma": "舞う",
              "pos": "动词",
              "meaning": "飘舞；飞舞。"
            },
            {
              "surface": "薄紅",
              "reading": "うすべに",
              "lemma": "薄紅",
              "pos": "名词",
              "meaning": "浅红色；淡红色。语境中可指淡红色的花瓣。"
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "、たしか",
              "reading": "",
              "lemma": "、たしか",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "美しく",
              "reading": "うつくしく",
              "lemma": "美しい",
              "pos": "形容词",
              "meaning": "美丽地；「美しい」的连用形。"
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "あった",
              "reading": "",
              "lemma": "あった",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "が",
              "reading": "",
              "lemma": "が",
              "pos": "助词",
              "meaning": "标记主语，或作为接续表示转折。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "俺",
              "reading": "おれ",
              "lemma": "俺",
              "pos": "代词",
              "meaning": "我。较随意的男性自称，语气依人物而定。"
            },
            {
              "surface": "には",
              "reading": "",
              "lemma": "には",
              "pos": "助词",
              "meaning": "场所或对象「に」与主题「は」的组合，依语境理解。"
            },
            {
              "surface": "ただそれだけ",
              "reading": "",
              "lemma": "ただそれだけ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "事",
              "reading": "こと",
              "lemma": "事",
              "pos": "名词",
              "meaning": "事情；事态。"
            },
            {
              "surface": "と",
              "reading": "",
              "lemma": "と",
              "pos": "助词",
              "meaning": "可表示共同对象、引用等，需结合语境。"
            },
            {
              "surface": "しか",
              "reading": "",
              "lemma": "しか",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "感じられない",
              "reading": "かんじられない",
              "lemma": "感じる",
              "pos": "动词",
              "meaning": "无法感到；也可随语境理解为感受不到。"
            },
            {
              "surface": "。",
              "reading": "",
              "lemma": "。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/sakura-03.png",
          "pageGroup": 3,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sakura-004",
          "gameId": "sakura",
          "source": "鼻につくような桜の香り――いや、桜にそんな強い芳香などない――の中を人々の手によって大事に運ばれる棺。",
          "rawSource": "鼻につくような桜の香り――いや、桜にそんな強い芳香などない――の中を人々の手によって大事に運ばれる棺。",
          "translation": "在那仿佛刺鼻的樱花香气——不，樱花其实并没有如此浓郁的芳香——之中，一口棺木被众人小心地抬着。",
          "tokens": [
            {
              "surface": "鼻",
              "reading": "",
              "lemma": "鼻",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "つくような",
              "reading": "",
              "lemma": "つくような",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "桜",
              "reading": "さくら",
              "lemma": "桜",
              "pos": "名词",
              "meaning": "樱花；樱树。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "香り",
              "reading": "かおり",
              "lemma": "香り",
              "pos": "名词",
              "meaning": "香气；气味。"
            },
            {
              "surface": "――いや、",
              "reading": "",
              "lemma": "――いや、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "桜",
              "reading": "さくら",
              "lemma": "桜",
              "pos": "名词",
              "meaning": "樱花；樱树。"
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "そんな強い",
              "reading": "",
              "lemma": "そんな強い",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "芳香",
              "reading": "ほうこう",
              "lemma": "芳香",
              "pos": "名词",
              "meaning": "芳香；香味。"
            },
            {
              "surface": "などない――",
              "reading": "",
              "lemma": "などない――",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "中",
              "reading": "",
              "lemma": "中",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "を",
              "reading": "",
              "lemma": "を",
              "pos": "助词",
              "meaning": "通常标记动作直接涉及的对象。"
            },
            {
              "surface": "人々",
              "reading": "ひとびと",
              "lemma": "人々",
              "pos": "名词",
              "meaning": "人们；众人。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "手",
              "reading": "",
              "lemma": "手",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "よって",
              "reading": "",
              "lemma": "よって",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "大事",
              "reading": "だいじ",
              "lemma": "大事",
              "pos": "名词",
              "meaning": "重要；珍重。在「大事に」中表示小心地、珍重地。"
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "運ばれる",
              "reading": "はこばれる",
              "lemma": "運ぶ",
              "pos": "动词",
              "meaning": "被搬运；「運ぶ」的被动形。"
            },
            {
              "surface": "棺",
              "reading": "ひつぎ",
              "lemma": "棺",
              "pos": "名词",
              "meaning": "棺木。"
            },
            {
              "surface": "。",
              "reading": "",
              "lemma": "。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/sakura-04.png",
          "pageGroup": 4,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sakura-005",
          "gameId": "sakura",
          "source": "桜の美しさの中にいる親父の死骸は、ただ、醜いだけだった。",
          "rawSource": "桜の美しさの中にいる親父の死骸は、ただ、醜いだけだった。",
          "translation": "置身樱花之美中的父亲遗体，只有丑陋而已。",
          "tokens": [
            {
              "surface": "桜",
              "reading": "さくら",
              "lemma": "桜",
              "pos": "名词",
              "meaning": "樱花；樱树。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "美しさ",
              "reading": "うつくしさ",
              "lemma": "美しい",
              "pos": "名词",
              "meaning": "美丽；「美しい」加「さ」名词化。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "中",
              "reading": "",
              "lemma": "中",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "いる",
              "reading": "",
              "lemma": "いる",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "親父",
              "reading": "おやじ",
              "lemma": "親父",
              "pos": "名词",
              "meaning": "老爸；父亲。较随意的称呼。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "死骸",
              "reading": "しがい",
              "lemma": "死骸",
              "pos": "名词",
              "meaning": "尸体；遗体。"
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "、ただ、",
              "reading": "",
              "lemma": "、ただ、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "醜い",
              "reading": "みにくい",
              "lemma": "醜い",
              "pos": "形容词",
              "meaning": "丑陋的。"
            },
            {
              "surface": "だけだった。",
              "reading": "",
              "lemma": "だけだった。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/sakura-05.png",
          "pageGroup": 5,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        }
      ],
      "path": "C:\\Games\\SakuraNoUta10th\\sakuranouta10th.exe"
    },
    {
      "id": "mo",
      "title": "Memories Off",
      "subtitle": "告别回忆",
      "exe": "Game.exe",
      "arch": "x86",
      "bottle": "Steam",
      "accent": "#b17b51",
      "cover": "assets/mo-01.png",
      "scene": "assets/mo-01.png",
      "language": "ja",
      "hook": "MAGES_text",
      "streams": [
        {
          "id": "mo-body",
          "label": "MAGES_text",
          "hook": "MAGES_text",
          "preview": "秋をひしひしと感じる季節になっていた。",
          "address": "4386260",
          "context": "0",
          "context2": "0",
          "kind": "正文候选"
        },
        {
          "id": "mo-secondary",
          "label": "MAGES_text",
          "hook": "MAGES_text",
          "preview": "セーブ　ロード　設定",
          "address": "4386292",
          "context": "1",
          "context2": "0",
          "kind": "界面文本候选"
        }
      ],
      "lines": [
        {
          "id": "mo-001",
          "gameId": "mo",
          "source": "秋をひしひしと感じる季節になっていた。",
          "rawSource": "秋をひしひしと感じる季節になっていた。",
          "translation": "已经到了深切感受到秋意的季节。",
          "tokens": [
            {
              "surface": "秋",
              "reading": "あき",
              "lemma": "秋",
              "pos": "名词",
              "meaning": "秋天。"
            },
            {
              "surface": "を",
              "reading": "",
              "lemma": "を",
              "pos": "助词",
              "meaning": "通常标记动作直接涉及的对象。"
            },
            {
              "surface": "ひしひしと",
              "reading": "",
              "lemma": "ひしひし",
              "pos": "副词",
              "meaning": "深切地；强烈地。"
            },
            {
              "surface": "感じる",
              "reading": "かんじる",
              "lemma": "感じる",
              "pos": "动词",
              "meaning": "感到；感觉到。"
            },
            {
              "surface": "季節",
              "reading": "きせつ",
              "lemma": "季節",
              "pos": "名词",
              "meaning": "季节。"
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "なっていた。",
              "reading": "",
              "lemma": "なっていた。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/mo-01.png",
          "pageGroup": 1,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "mo-002",
          "gameId": "mo",
          "source": "いつもと何ら変わりない生活。",
          "rawSource": "いつもと何ら変わりない生活。",
          "translation": "一如既往、毫无变化的生活。",
          "tokens": [
            {
              "surface": "いつも",
              "reading": "",
              "lemma": "いつも",
              "pos": "副词",
              "meaning": "总是；平时。"
            },
            {
              "surface": "と",
              "reading": "",
              "lemma": "と",
              "pos": "助词",
              "meaning": "可表示共同对象、引用等，需结合语境。"
            },
            {
              "surface": "何ら",
              "reading": "なんら",
              "lemma": "何ら",
              "pos": "副词",
              "meaning": "丝毫；与否定形式呼应。"
            },
            {
              "surface": "変わりない",
              "reading": "かわりない",
              "lemma": "変わりない",
              "pos": "形容词",
              "meaning": "没有变化；一如往常。"
            },
            {
              "surface": "生活",
              "reading": "せいかつ",
              "lemma": "生活",
              "pos": "名词",
              "meaning": "生活。"
            },
            {
              "surface": "。",
              "reading": "",
              "lemma": "。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/mo-02.png",
          "pageGroup": 2,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "mo-003",
          "gameId": "mo",
          "source": "隣りの幼なじみに起こされ、けだるい体で家を出る。",
          "rawSource": "隣りの幼なじみに起こされ、けだるい体で家を出る。",
          "translation": "被住在隔壁的青梅竹马叫醒，拖着倦怠的身体走出家门。",
          "tokens": [
            {
              "surface": "隣り",
              "reading": "となり",
              "lemma": "隣り",
              "pos": "名词",
              "meaning": "隔壁；旁边。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "幼なじみ",
              "reading": "おさななじみ",
              "lemma": "幼なじみ",
              "pos": "名词",
              "meaning": "青梅竹马；从小相识的人。"
            },
            {
              "surface": "に",
              "reading": "",
              "lemma": "に",
              "pos": "助词",
              "meaning": "标记时间、对象、归着点等，需结合句子。"
            },
            {
              "surface": "起こされ",
              "reading": "おこされ",
              "lemma": "起こす",
              "pos": "动词",
              "meaning": "被叫醒；「起こす」的被动连用形式。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "けだるい",
              "reading": "",
              "lemma": "けだるい",
              "pos": "形容词",
              "meaning": "倦怠的；懒洋洋的。"
            },
            {
              "surface": "体",
              "reading": "からだ",
              "lemma": "体",
              "pos": "名词",
              "meaning": "身体。"
            },
            {
              "surface": "で",
              "reading": "",
              "lemma": "で",
              "pos": "助词",
              "meaning": "可标记动作场所、方式或条件，需结合语境。"
            },
            {
              "surface": "家",
              "reading": "いえ",
              "lemma": "家",
              "pos": "名词",
              "meaning": "家；房屋。"
            },
            {
              "surface": "を",
              "reading": "",
              "lemma": "を",
              "pos": "助词",
              "meaning": "通常标记动作直接涉及的对象。"
            },
            {
              "surface": "出る",
              "reading": "でる",
              "lemma": "出る",
              "pos": "动词",
              "meaning": "出去；离开。"
            },
            {
              "surface": "。",
              "reading": "",
              "lemma": "。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/mo-03.png",
          "pageGroup": 3,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "mo-004",
          "gameId": "mo",
          "source": "【智也】「……お待たせ」",
          "rawSource": "【智也】「……お待たせ」",
          "translation": "【智也】「……久等了。」",
          "tokens": [
            {
              "surface": "【智也】「……",
              "reading": "",
              "lemma": "【智也】「……",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "お待たせ",
              "reading": "おまたせ",
              "lemma": "待たせる",
              "pos": "表达",
              "meaning": "让你久等了。"
            },
            {
              "surface": "」",
              "reading": "",
              "lemma": "」",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/mo-04.png",
          "pageGroup": 4,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        }
      ],
      "path": "C:\\Program Files (x86)\\Steam\\steamapps\\common\\memo01\\Game.exe"
    },
    {
      "id": "never7",
      "title": "Never7 -the end of infinity-",
      "subtitle": "Never7",
      "exe": "GameN7_Steam.exe",
      "arch": "x64",
      "bottle": "Steam",
      "accent": "#4c76a8",
      "cover": "assets/never7-01.png",
      "scene": "assets/never7-01.png",
      "language": "ja",
      "hook": "MAGES2",
      "streams": [
        {
          "id": "never7-body",
          "label": "MAGES2",
          "hook": "MAGES2",
          "preview": "それはオレの言葉だったのだろうか？",
          "address": "5369427648",
          "context": "718481",
          "context2": "0",
          "kind": "正文候选"
        },
        {
          "id": "never7-secondary",
          "label": "MAGES2",
          "hook": "MAGES2",
          "preview": "セーブ　ロード　設定",
          "address": "5369427680",
          "context": "1",
          "context2": "0",
          "kind": "界面文本候选"
        }
      ],
      "lines": [
        {
          "id": "never7-001",
          "gameId": "never7",
          "source": "それはオレの言葉だったのだろうか？",
          "rawSource": "それはオレの言葉だったのだろうか？\n",
          "translation": "那是我说的话吗？",
          "tokens": [
            {
              "surface": "それ",
              "reading": "",
              "lemma": "それ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "オレ",
              "reading": "",
              "lemma": "俺",
              "pos": "代词",
              "meaning": "我。片假名写法，语气依人物而定。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "言葉",
              "reading": "ことば",
              "lemma": "言葉",
              "pos": "名词",
              "meaning": "语言；话语。"
            },
            {
              "surface": "だった",
              "reading": "",
              "lemma": "だった",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "のだろうか",
              "reading": "",
              "lemma": "のだろうか",
              "pos": "表达",
              "meaning": "表示推测性的疑问或自问。"
            },
            {
              "surface": "？",
              "reading": "",
              "lemma": "？",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/never7-01.png",
          "pageGroup": 1,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "never7-002",
          "gameId": "never7",
          "source": "それはオレの言葉だったのだろうか？\nあるいは……別の誰かの？",
          "rawSource": "あるいは……別の誰かの？\n",
          "translation": "那是我说的话吗？\n又或者……是别的谁说的？",
          "tokens": [
            {
              "surface": "それ",
              "reading": "",
              "lemma": "それ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "オレ",
              "reading": "",
              "lemma": "俺",
              "pos": "代词",
              "meaning": "我。片假名写法，语气依人物而定。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "言葉",
              "reading": "ことば",
              "lemma": "言葉",
              "pos": "名词",
              "meaning": "语言；话语。"
            },
            {
              "surface": "だった",
              "reading": "",
              "lemma": "だった",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "のだろうか",
              "reading": "",
              "lemma": "のだろうか",
              "pos": "表达",
              "meaning": "表示推测性的疑问或自问。"
            },
            {
              "surface": "？\nあるい",
              "reading": "",
              "lemma": "？\nあるい",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "……別",
              "reading": "",
              "lemma": "……別",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "誰か",
              "reading": "だれか",
              "lemma": "誰か",
              "pos": "代词",
              "meaning": "某个人；谁。"
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "？",
              "reading": "",
              "lemma": "？",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/never7-02.png",
          "pageGroup": 1,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "never7-003",
          "gameId": "never7",
          "source": "いや、今となっては、そもそもそれが『言葉』だったのかどうかさえ疑わしい。",
          "rawSource": "いや、今となっては、そもそもそれが『言葉』だったのかどうかさえ疑わしい。\n",
          "translation": "不，到了现在，就连那究竟算不算“话语”都令人怀疑。",
          "tokens": [
            {
              "surface": "いや、",
              "reading": "",
              "lemma": "いや、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "今",
              "reading": "いま",
              "lemma": "今",
              "pos": "名词",
              "meaning": "现在。"
            },
            {
              "surface": "と",
              "reading": "",
              "lemma": "と",
              "pos": "助词",
              "meaning": "可表示共同对象、引用等，需结合语境。"
            },
            {
              "surface": "なって",
              "reading": "",
              "lemma": "なって",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "は",
              "reading": "",
              "lemma": "は",
              "pos": "助词",
              "meaning": "提示主题，也可表示对比。"
            },
            {
              "surface": "、そ",
              "reading": "",
              "lemma": "、そ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "も",
              "reading": "",
              "lemma": "も",
              "pos": "助词",
              "meaning": "也；与否定呼应时表示都不。"
            },
            {
              "surface": "そ",
              "reading": "",
              "lemma": "そ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "も",
              "reading": "",
              "lemma": "も",
              "pos": "助词",
              "meaning": "也；与否定呼应时表示都不。"
            },
            {
              "surface": "それ",
              "reading": "",
              "lemma": "それ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "が",
              "reading": "",
              "lemma": "が",
              "pos": "助词",
              "meaning": "标记主语，或作为接续表示转折。"
            },
            {
              "surface": "『",
              "reading": "",
              "lemma": "『",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "言葉",
              "reading": "ことば",
              "lemma": "言葉",
              "pos": "名词",
              "meaning": "语言；话语。"
            },
            {
              "surface": "』だった",
              "reading": "",
              "lemma": "』だった",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "の",
              "reading": "",
              "lemma": "の",
              "pos": "助词",
              "meaning": "连接修饰关系，或用于名词化，需结合语境。"
            },
            {
              "surface": "かどうかさえ",
              "reading": "",
              "lemma": "かどうかさえ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "疑わしい",
              "reading": "うたがわしい",
              "lemma": "疑わしい",
              "pos": "形容词",
              "meaning": "可疑的；令人怀疑的。"
            },
            {
              "surface": "。",
              "reading": "",
              "lemma": "。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/never7-03.png",
          "pageGroup": 2,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "never7-004",
          "gameId": "never7",
          "source": "ここには何も無かった。",
          "rawSource": "ここには何も無かった。\n",
          "translation": "这里什么都没有。",
          "tokens": [
            {
              "surface": "ここ",
              "reading": "",
              "lemma": "ここ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "には",
              "reading": "",
              "lemma": "には",
              "pos": "助词",
              "meaning": "场所或对象「に」与主题「は」的组合，依语境理解。"
            },
            {
              "surface": "何も",
              "reading": "なにも",
              "lemma": "何も",
              "pos": "表达",
              "meaning": "与否定呼应，表示什么都不。"
            },
            {
              "surface": "無かった",
              "reading": "なかった",
              "lemma": "無い",
              "pos": "形容词",
              "meaning": "没有；「無い」的过去形式。"
            },
            {
              "surface": "。",
              "reading": "",
              "lemma": "。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/never7-04.png",
          "pageGroup": 3,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "never7-005",
          "gameId": "never7",
          "source": "ここには何も無かった。\n音も無く、色も無く、風も無く……。",
          "rawSource": "音も無く、色も無く、風も無く……。\n",
          "translation": "这里什么都没有。\n没有声音，没有颜色，也没有风……。",
          "tokens": [
            {
              "surface": "ここ",
              "reading": "",
              "lemma": "ここ",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "には",
              "reading": "",
              "lemma": "には",
              "pos": "助词",
              "meaning": "场所或对象「に」与主题「は」的组合，依语境理解。"
            },
            {
              "surface": "何も",
              "reading": "なにも",
              "lemma": "何も",
              "pos": "表达",
              "meaning": "与否定呼应，表示什么都不。"
            },
            {
              "surface": "無かった",
              "reading": "なかった",
              "lemma": "無い",
              "pos": "形容词",
              "meaning": "没有；「無い」的过去形式。"
            },
            {
              "surface": "。\n",
              "reading": "",
              "lemma": "。\n",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "音",
              "reading": "おと",
              "lemma": "音",
              "pos": "名词",
              "meaning": "声音。"
            },
            {
              "surface": "も",
              "reading": "",
              "lemma": "も",
              "pos": "助词",
              "meaning": "也；与否定呼应时表示都不。"
            },
            {
              "surface": "無く",
              "reading": "なく",
              "lemma": "無い",
              "pos": "形容词",
              "meaning": "没有；「無い」的连用形。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "色",
              "reading": "いろ",
              "lemma": "色",
              "pos": "名词",
              "meaning": "颜色。"
            },
            {
              "surface": "も",
              "reading": "",
              "lemma": "も",
              "pos": "助词",
              "meaning": "也；与否定呼应时表示都不。"
            },
            {
              "surface": "無く",
              "reading": "なく",
              "lemma": "無い",
              "pos": "形容词",
              "meaning": "没有；「無い」的连用形。"
            },
            {
              "surface": "、",
              "reading": "",
              "lemma": "、",
              "pos": "原文",
              "meaning": ""
            },
            {
              "surface": "風",
              "reading": "かぜ",
              "lemma": "風",
              "pos": "名词",
              "meaning": "风。"
            },
            {
              "surface": "も",
              "reading": "",
              "lemma": "も",
              "pos": "助词",
              "meaning": "也；与否定呼应时表示都不。"
            },
            {
              "surface": "無く",
              "reading": "なく",
              "lemma": "無い",
              "pos": "形容词",
              "meaning": "没有；「無い」的连用形。"
            },
            {
              "surface": "……。",
              "reading": "",
              "lemma": "……。",
              "pos": "原文",
              "meaning": ""
            }
          ],
          "language": "ja",
          "scene": "assets/never7-05.png",
          "pageGroup": 3,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        }
      ],
      "path": "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Never7\\GameN7_Steam.exe"
    },
    {
      "id": "sr",
      "title": "Symphonic Rain",
      "subtitle": "交响乐之雨 · 中文版",
      "exe": "SR.exe",
      "arch": "x86",
      "bottle": "Steam",
      "accent": "#588a7d",
      "cover": "assets/sr-01.png",
      "scene": "assets/sr-01.png",
      "language": "zh-CN",
      "hook": "TextOutW",
      "streams": [
        {
          "id": "sr-body",
          "label": "TextOutW",
          "hook": "TextOutW",
          "preview": "【法珞】「原来是这样。啊……那样子叫我太见外了。所以请叫我法珞就好了。和我亲近的人都这样叫我。」",
          "address": "2070416512",
          "context": "49923",
          "context2": "0",
          "kind": "正文候选"
        },
        {
          "id": "sr-secondary",
          "label": "TextOutW",
          "hook": "TextOutW",
          "preview": "セーブ　ロード　設定",
          "address": "2070416544",
          "context": "1",
          "context2": "0",
          "kind": "界面文本候选"
        }
      ],
      "lines": [
        {
          "id": "sr-001",
          "gameId": "sr",
          "source": "【法珞】「原来是这样。啊……那样子叫我太见外了。所以请叫我法珞就好了。和我亲近的人都这样叫我。」",
          "rawSource": "【法珞】「原来是这样。啊……那样子叫我太见外了。所以请叫我法珞就好了。和我亲近的人都这样叫我。」",
          "translation": "",
          "tokens": [
            {
              "surface": "【法珞】「原来是这样。啊……那样子叫我太见外了。所以请叫我法珞就好了。和我亲近的人都这样叫我。」",
              "reading": "",
              "lemma": "【法珞】「原来是这样。啊……那样子叫我太见外了。所以请叫我法珞就好了。和我亲近的人都这样叫我。」",
              "pos": "中文原文",
              "meaning": ""
            }
          ],
          "language": "zh-CN",
          "scene": "assets/sr-01.png",
          "pageGroup": 1,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sr-002",
          "gameId": "sr",
          "source": "【库里斯】「法珞……同学？」",
          "rawSource": "【库里斯】「法珞……同学？」",
          "translation": "",
          "tokens": [
            {
              "surface": "【库里斯】「法珞……同学？」",
              "reading": "",
              "lemma": "【库里斯】「法珞……同学？」",
              "pos": "中文原文",
              "meaning": ""
            }
          ],
          "language": "zh-CN",
          "scene": "assets/sr-02.png",
          "pageGroup": 2,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sr-003",
          "gameId": "sr",
          "source": "虽然不认为从此以后我就真的能变成跟她亲近的人，但是被她这么一说，我也没有什么需要拒绝的理由。",
          "rawSource": "虽然不认为从此以后我就真的能变成跟她亲近的人，但是被她这么一说，我也没有什么需要拒绝的理由。",
          "translation": "",
          "tokens": [
            {
              "surface": "虽然不认为从此以后我就真的能变成跟她亲近的人，但是被她这么一说，我也没有什么需要拒绝的理由。",
              "reading": "",
              "lemma": "虽然不认为从此以后我就真的能变成跟她亲近的人，但是被她这么一说，我也没有什么需要拒绝的理由。",
              "pos": "中文原文",
              "meaning": ""
            }
          ],
          "language": "zh-CN",
          "scene": "assets/sr-03.png",
          "pageGroup": 3,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sr-004",
          "gameId": "sr",
          "source": "虽然这样，总是有点不习惯……所以忍不住还是在后面加了个「同学」。",
          "rawSource": "虽然这样，总是有点不习惯……所以忍不住还是在后面加了个「同学」。",
          "translation": "",
          "tokens": [
            {
              "surface": "虽然这样，总是有点不习惯……所以忍不住还是在后面加了个「同学」。",
              "reading": "",
              "lemma": "虽然这样，总是有点不习惯……所以忍不住还是在后面加了个「同学」。",
              "pos": "中文原文",
              "meaning": ""
            }
          ],
          "language": "zh-CN",
          "scene": "assets/sr-04.png",
          "pageGroup": 4,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        },
        {
          "id": "sr-005",
          "gameId": "sr",
          "source": "【法珞】「嗯」",
          "rawSource": "【法珞】「嗯」",
          "translation": "",
          "tokens": [
            {
              "surface": "【法珞】「嗯」",
              "reading": "",
              "lemma": "【法珞】「嗯」",
              "pos": "中文原文",
              "meaning": ""
            }
          ],
          "language": "zh-CN",
          "scene": "assets/sr-05.png",
          "pageGroup": 5,
          "explanation": {
            "summary": "模型解释需结合本句与允许的上下文核对。以下内容为人工编写的设计样本。"
          }
        }
      ],
      "path": "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Symphonic Rain\\SR.exe"
    }
  ],
  "manualSample": {
    "id": "manual-001",
    "gameId": "manual",
    "source": "放課後、図書館で彼女と待ち合わせをした。",
    "translation": "放学后，我和她约好在图书馆见面。",
    "language": "ja",
    "tokens": [
      {
        "surface": "放課後",
        "reading": "ほうかご",
        "lemma": "放課後",
        "pos": "名词",
        "meaning": "放学后。"
      },
      {
        "surface": "、",
        "reading": "",
        "lemma": "、",
        "pos": "原文",
        "meaning": ""
      },
      {
        "surface": "図書館",
        "reading": "としょかん",
        "lemma": "図書館",
        "pos": "名词",
        "meaning": "图书馆。"
      },
      {
        "surface": "で",
        "reading": "",
        "lemma": "で",
        "pos": "助词",
        "meaning": "可标记动作场所、方式或条件，需结合语境。"
      },
      {
        "surface": "彼女",
        "reading": "かのじょ",
        "lemma": "彼女",
        "pos": "代词",
        "meaning": "她；有时指女朋友，需结合语境。"
      },
      {
        "surface": "と",
        "reading": "",
        "lemma": "と",
        "pos": "助词",
        "meaning": "可表示共同对象、引用等，需结合语境。"
      },
      {
        "surface": "待ち合わせ",
        "reading": "まちあわせ",
        "lemma": "待ち合わせ",
        "pos": "名词",
        "meaning": "约定会面；碰头。"
      },
      {
        "surface": "を",
        "reading": "",
        "lemma": "を",
        "pos": "助词",
        "meaning": "通常标记动作直接涉及的对象。"
      },
      {
        "surface": "した",
        "reading": "",
        "lemma": "する",
        "pos": "动词",
        "meaning": "做了；「する」的过去形。"
      },
      {
        "surface": "。",
        "reading": "",
        "lemma": "。",
        "pos": "原文",
        "meaning": ""
      }
    ],
    "explanation": {
      "summary": "原句省略了主语；中文“我”属于结合语境的自然补充。"
    }
  },
  "lexicon": [
    {
      "surface": "泣かなかった",
      "reading": "なかなかった",
      "lemma": "泣く",
      "pos": "动词",
      "meaning": "没有哭。「泣く」的否定过去形。"
    },
    {
      "surface": "感じられない",
      "reading": "かんじられない",
      "lemma": "感じる",
      "pos": "动词",
      "meaning": "无法感到；也可随语境理解为感受不到。"
    },
    {
      "surface": "のだろうか",
      "reading": "のだろうか",
      "lemma": "のだろうか",
      "pos": "表达",
      "meaning": "表示推测性的疑问或自问。"
    },
    {
      "surface": "ひしひしと",
      "reading": "ひしひしと",
      "lemma": "ひしひし",
      "pos": "副词",
      "meaning": "深切地；强烈地。"
    },
    {
      "surface": "変わりない",
      "reading": "かわりない",
      "lemma": "変わりない",
      "pos": "形容词",
      "meaning": "没有变化；一如往常。"
    },
    {
      "surface": "待ち合わせ",
      "reading": "まちあわせ",
      "lemma": "待ち合わせ",
      "pos": "名词",
      "meaning": "约定会面；碰头。"
    },
    {
      "surface": "なかった",
      "reading": "なかった",
      "lemma": "ない",
      "pos": "助动词",
      "meaning": "「ない」的过去形式；没有。"
    },
    {
      "surface": "運ばれる",
      "reading": "はこばれる",
      "lemma": "運ぶ",
      "pos": "动词",
      "meaning": "被搬运；「運ぶ」的被动形。"
    },
    {
      "surface": "幼なじみ",
      "reading": "おさななじみ",
      "lemma": "幼なじみ",
      "pos": "名词",
      "meaning": "青梅竹马；从小相识的人。"
    },
    {
      "surface": "起こされ",
      "reading": "おこされ",
      "lemma": "起こす",
      "pos": "动词",
      "meaning": "被叫醒；「起こす」的被动连用形式。"
    },
    {
      "surface": "けだるい",
      "reading": "けだるい",
      "lemma": "けだるい",
      "pos": "形容词",
      "meaning": "倦怠的；懒洋洋的。"
    },
    {
      "surface": "お待たせ",
      "reading": "おまたせ",
      "lemma": "待たせる",
      "pos": "表达",
      "meaning": "让你久等了。"
    },
    {
      "surface": "疑わしい",
      "reading": "うたがわしい",
      "lemma": "疑わしい",
      "pos": "形容词",
      "meaning": "可疑的；令人怀疑的。"
    },
    {
      "surface": "無かった",
      "reading": "なかった",
      "lemma": "無い",
      "pos": "形容词",
      "meaning": "没有；「無い」的过去形式。"
    },
    {
      "surface": "悲しさ",
      "reading": "かなしさ",
      "lemma": "悲しい",
      "pos": "名词",
      "meaning": "悲伤；悲伤的程度。由形容词「悲しい」加「さ」名词化。"
    },
    {
      "surface": "だから",
      "reading": "だから",
      "lemma": "だから",
      "pos": "接续词",
      "meaning": "因此；所以。"
    },
    {
      "surface": "美しく",
      "reading": "うつくしく",
      "lemma": "美しい",
      "pos": "形容词",
      "meaning": "美丽地；「美しい」的连用形。"
    },
    {
      "surface": "美しさ",
      "reading": "うつくしさ",
      "lemma": "美しい",
      "pos": "名词",
      "meaning": "美丽；「美しい」加「さ」名词化。"
    },
    {
      "surface": "感じる",
      "reading": "かんじる",
      "lemma": "感じる",
      "pos": "动词",
      "meaning": "感到；感觉到。"
    },
    {
      "surface": "いつも",
      "reading": "いつも",
      "lemma": "いつも",
      "pos": "副词",
      "meaning": "总是；平时。"
    },
    {
      "surface": "放課後",
      "reading": "ほうかご",
      "lemma": "放課後",
      "pos": "名词",
      "meaning": "放学后。"
    },
    {
      "surface": "図書館",
      "reading": "としょかん",
      "lemma": "図書館",
      "pos": "名词",
      "meaning": "图书馆。"
    },
    {
      "surface": "その",
      "reading": "その",
      "lemma": "その",
      "pos": "连体词",
      "meaning": "那个；用于指代前文或双方已知的对象。"
    },
    {
      "surface": "風景",
      "reading": "ふうけい",
      "lemma": "風景",
      "pos": "名词",
      "meaning": "风景；景色。"
    },
    {
      "surface": "痛み",
      "reading": "いたみ",
      "lemma": "痛み",
      "pos": "名词",
      "meaning": "疼痛；痛楚。"
    },
    {
      "surface": "舞う",
      "reading": "まう",
      "lemma": "舞う",
      "pos": "动词",
      "meaning": "飘舞；飞舞。"
    },
    {
      "surface": "薄紅",
      "reading": "うすべに",
      "lemma": "薄紅",
      "pos": "名词",
      "meaning": "浅红色；淡红色。语境中可指淡红色的花瓣。"
    },
    {
      "surface": "香り",
      "reading": "かおり",
      "lemma": "香り",
      "pos": "名词",
      "meaning": "香气；气味。"
    },
    {
      "surface": "芳香",
      "reading": "ほうこう",
      "lemma": "芳香",
      "pos": "名词",
      "meaning": "芳香；香味。"
    },
    {
      "surface": "人々",
      "reading": "ひとびと",
      "lemma": "人々",
      "pos": "名词",
      "meaning": "人们；众人。"
    },
    {
      "surface": "大事",
      "reading": "だいじ",
      "lemma": "大事",
      "pos": "名词",
      "meaning": "重要；珍重。在「大事に」中表示小心地、珍重地。"
    },
    {
      "surface": "親父",
      "reading": "おやじ",
      "lemma": "親父",
      "pos": "名词",
      "meaning": "老爸；父亲。较随意的称呼。"
    },
    {
      "surface": "死骸",
      "reading": "しがい",
      "lemma": "死骸",
      "pos": "名词",
      "meaning": "尸体；遗体。"
    },
    {
      "surface": "醜い",
      "reading": "みにくい",
      "lemma": "醜い",
      "pos": "形容词",
      "meaning": "丑陋的。"
    },
    {
      "surface": "季節",
      "reading": "きせつ",
      "lemma": "季節",
      "pos": "名词",
      "meaning": "季节。"
    },
    {
      "surface": "何ら",
      "reading": "なんら",
      "lemma": "何ら",
      "pos": "副词",
      "meaning": "丝毫；与否定形式呼应。"
    },
    {
      "surface": "生活",
      "reading": "せいかつ",
      "lemma": "生活",
      "pos": "名词",
      "meaning": "生活。"
    },
    {
      "surface": "隣り",
      "reading": "となり",
      "lemma": "隣り",
      "pos": "名词",
      "meaning": "隔壁；旁边。"
    },
    {
      "surface": "出る",
      "reading": "でる",
      "lemma": "出る",
      "pos": "动词",
      "meaning": "出去；离开。"
    },
    {
      "surface": "言葉",
      "reading": "ことば",
      "lemma": "言葉",
      "pos": "名词",
      "meaning": "语言；话语。"
    },
    {
      "surface": "オレ",
      "reading": "おれ",
      "lemma": "俺",
      "pos": "代词",
      "meaning": "我。片假名写法，语气依人物而定。"
    },
    {
      "surface": "誰か",
      "reading": "だれか",
      "lemma": "誰か",
      "pos": "代词",
      "meaning": "某个人；谁。"
    },
    {
      "surface": "何も",
      "reading": "なにも",
      "lemma": "何も",
      "pos": "表达",
      "meaning": "与否定呼应，表示什么都不。"
    },
    {
      "surface": "無く",
      "reading": "なく",
      "lemma": "無い",
      "pos": "形容词",
      "meaning": "没有；「無い」的连用形。"
    },
    {
      "surface": "彼女",
      "reading": "かのじょ",
      "lemma": "彼女",
      "pos": "代词",
      "meaning": "她；有时指女朋友，需结合语境。"
    },
    {
      "surface": "した",
      "reading": "した",
      "lemma": "する",
      "pos": "动词",
      "meaning": "做了；「する」的过去形。"
    },
    {
      "surface": "俺",
      "reading": "おれ",
      "lemma": "俺",
      "pos": "代词",
      "meaning": "我。较随意的男性自称，语气依人物而定。"
    },
    {
      "surface": "風",
      "reading": "かぜ",
      "lemma": "風",
      "pos": "名词",
      "meaning": "风。"
    },
    {
      "surface": "事",
      "reading": "こと",
      "lemma": "事",
      "pos": "名词",
      "meaning": "事情；事态。"
    },
    {
      "surface": "桜",
      "reading": "さくら",
      "lemma": "桜",
      "pos": "名词",
      "meaning": "樱花；樱树。"
    },
    {
      "surface": "棺",
      "reading": "ひつぎ",
      "lemma": "棺",
      "pos": "名词",
      "meaning": "棺木。"
    },
    {
      "surface": "秋",
      "reading": "あき",
      "lemma": "秋",
      "pos": "名词",
      "meaning": "秋天。"
    },
    {
      "surface": "体",
      "reading": "からだ",
      "lemma": "体",
      "pos": "名词",
      "meaning": "身体。"
    },
    {
      "surface": "家",
      "reading": "いえ",
      "lemma": "家",
      "pos": "名词",
      "meaning": "家；房屋。"
    },
    {
      "surface": "今",
      "reading": "いま",
      "lemma": "今",
      "pos": "名词",
      "meaning": "现在。"
    },
    {
      "surface": "音",
      "reading": "おと",
      "lemma": "音",
      "pos": "名词",
      "meaning": "声音。"
    },
    {
      "surface": "色",
      "reading": "いろ",
      "lemma": "色",
      "pos": "名词",
      "meaning": "颜色。"
    }
  ],
  "provenance": "Game scenes and source strings are paired previous local Core probe evidence. Translations, token annotations and explanations are authored design fixtures, not live backend results."
};

// Explicit offline preview facts; unknown native facts remain unprovided.
window.LunaDesignData.previewDesign = {
  provenance: '离线设计样例；场景来自既有采集素材，不是当前系统窗口或本轮原生输出',
  privacy: '只预览已明确选择并核对的游戏窗口，不采音频、不上传。持续画面不录制、不连续保存；仅在正式关联、已有录屏权限且游戏没有本机封面时保存首张有效帧为静态封面，不覆盖已有封面。会话持有预览，切换正文、全部提取或 Hook 日志页签不停止采集。预览与 Hook 取文独立；刷新不激活游戏，不改变窗口标题、位置、宽高、DPI 或图形设置。',
  modes: {
    available: {label:'自动预览 · 回放',guidance:'自动更新所选窗口的画面。',reason:'frame_ready',stage:'capture_complete',api:'SCStream · 设计样例'},
    starting: {label:'正在开启',empty:'正在连接采集器',guidance:'等待有效画面；游戏取文继续。',reason:'waiting_for_frame',stage:'capture_start',api:'SCStream.startCapture · 设计样例'},
    permission: {label:'需要授权',empty:'需要屏幕录制权限',guidance:'允许 Luna 录屏后自动恢复预览。',action:'系统设置…',recovery:'permission',reason:'screen_recording_denied',stage:'permission_check',api:'CGPreflightScreenCaptureAccess · 设计样例'},
    unavailable: {label:'暂不可用',empty:'窗口当前不可采集',guidance:'恢复游戏窗口，或重新选择窗口。',action:'选择窗口…',recovery:'picker',reason:'window_not_on_screen',stage:'capture_complete',api:'shareable_content · 设计样例'},
    exited: {label:'目标窗口已关闭',empty:'预览已停止',guidance:'重新打开游戏后，再选择运行窗口。',action:'重新关联…',recovery:'exited',reason:'target_exited',stage:'target_lifecycle',api:'process_identity · 设计样例'},
    identity_error: {label:'身份待核对',empty:'窗口身份已变化',guidance:'重新核对游戏窗口，避免显示错窗。',action:'重新核对…',recovery:'identity',reason:'owner_image_mapping_failed',stage:'owner_image_mapping',api:'shareable_content · 设计样例'}
  },
  stress: {
    long_title: {mode:'available',title:'サクラノ詩 Re:Graphic 10th Anniversary edition ― 特別限定パッケージ・追加ストーリー収録版（日本語）'},
    long_error: {mode:'unavailable',raw:'window_not_on_screen · capture_complete · picker_update_processed#2 / received=1 / processed=1 · API=shareable_content\n現在ゲームウインドウが画面上に見つかりません。スリープ復帰後、Stage Manager に収納されたウインドウ、最小化されたウインドウ、または別の Space に移動したウインドウの可能性があります。ウインドウを復元してから再試行するか、システムのウインドウ選択で対象を再確認してください。\n失败不更新原采集时间，不触发 Hook 重注入；完整路径和本次运行身份仍需逐项核对。'},
    japanese: {mode:'unavailable',title:'サクラノ詩 ― 春の空を彩る物語',guidance:'ゲーム窓を復元して再試行してください。',raw:'現在の画面を取得できません。最後の画像は元の取得時刻のまま保持します。ウインドウのタイトル、位置、大きさは変更しません。'}
  }
};
