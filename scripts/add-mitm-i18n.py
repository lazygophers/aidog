#!/usr/bin/env python3
"""Add mitm.* + common.{yes,no,add} + appSettings.mitmTab keys to all 8 locales.

Idempotent: skips keys already present (preserves existing translations).
Run once for ST7. Safe to re-run.
"""
import json
import sys
from pathlib import Path

LOCALES = ["zh-CN", "en-US", "ar-SA", "fr-FR", "de-DE", "ru-RU", "ja-JP", "es-ES"]

# key -> {locale: translation}
TRANS = {
    "appSettings.mitmTab": {
        "zh-CN": "MITM 解密",
        "en-US": "MITM Decryption",
        "ar-SA": "فك تشفير MITM",
        "fr-FR": "Déchiffrement MITM",
        "de-DE": "MITM-Entschlüsselung",
        "ru-RU": "Расшифровка MITM",
        "ja-JP": "MITM 復号",
        "es-ES": "Descifrado MITM",
    },
    "common.yes": {
        "zh-CN": "是", "en-US": "Yes", "ar-SA": "نعم", "fr-FR": "Oui",
        "de-DE": "Ja", "ru-RU": "Да", "ja-JP": "はい", "es-ES": "Sí",
    },
    "common.no": {
        "zh-CN": "否", "en-US": "No", "ar-SA": "لا", "fr-FR": "Non",
        "de-DE": "Nein", "ru-RU": "Нет", "ja-JP": "いいえ", "es-ES": "No",
    },
    "common.add": {
        "zh-CN": "添加", "en-US": "Add", "ar-SA": "إضافة", "fr-FR": "Ajouter",
        "de-DE": "Hinzufügen", "ru-RU": "Добавить", "ja-JP": "追加", "es-ES": "Añadir",
    },
    "mitm.masterToggle": {
        "zh-CN": "MITM 解密隧道",
        "en-US": "MITM Decryption Tunnel",
        "ar-SA": "نفق فك تشفير MITM",
        "fr-FR": "Tunnel de déchiffrement MITM",
        "de-DE": "MITM-Entschlüsselungstunnel",
        "ru-RU": "Туннель расшифровки MITM",
        "ja-JP": "MITM 復号トンネル",
        "es-ES": "Túnel de descifrado MITM",
    },
    "mitm.masterToggleDesc": {
        "zh-CN": "启用后 CONNECT 隧道内 HTTPS 流量经 AirDog 解密采集（需装假 CA）",
        "en-US": "When enabled, HTTPS traffic inside CONNECT tunnels is decrypted and captured by AirDog (requires installing a fake CA)",
        "ar-SA": "عند التمكين، يتم فك تشفير حركة HTTPS داخل أنفاق CONNECT والتقاطها بواسطة AirDog (يتطلب تثبيت CA وهمية)",
        "fr-FR": "Si activé, le trafic HTTPS dans les tunnels CONNECT est déchiffré et capturé par AirDog (nécessite l'installation d'un faux CA)",
        "de-DE": "Wenn aktiviert, wird der HTTPS-Verkehr innerhalb von CONNECT-Tunneln von AirDog entschlüsselt und erfasst (erfordert die Installation eines gefälschten CA)",
        "ru-RU": "Если включено, HTTPS-трафик внутри туннелей CONNECT расшифровывается и перехватывается AirDog (требуется установка поддельного УЦ)",
        "ja-JP": "有効にすると、CONNECT トンネル内の HTTPS トラフィックが AirDog によって復号・キャプチャされます（偽 CA のインストールが必要）",
        "es-ES": "Si está activado, el tráfico HTTPS dentro de los túneles CONNECT es descifrado y capturado por AirDog (requiere instalar un CA falso)",
    },
    "mitm.riskTitle": {
        "zh-CN": "安全提示",
        "en-US": "Security Notice",
        "ar-SA": "إشعار أمني",
        "fr-FR": "Avis de sécurité",
        "de-DE": "Sicherheitshinweis",
        "ru-RU": "Уведомление о безопасности",
        "ja-JP": "セキュリティ通知",
        "es-ES": "Aviso de seguridad",
    },
    "mitm.riskDesc": {
        "zh-CN": "假 CA 私钥明文存于本机数据库；私钥泄露 = 白名单内 HTTPS 可被解密。仅启用必要 host。",
        "en-US": "The fake CA private key is stored in plaintext in the local database; if leaked, whitelisted HTTPS can be decrypted. Enable only necessary hosts.",
        "ar-SA": "يتم تخزين المفتاح الخاص لـ CA الوهمية كنص عادي في قاعدة البيانات المحلية؛ إذا تم تسريبه، يمكن فك تشفير HTTPS المدرج في القائمة البيضاء. قم بتمكين المضيفات الضرورية فقط.",
        "fr-FR": "La clé privée du faux CA est stockée en clair dans la base de données locale ; en cas de fuite, les HTTPS en liste blanche peuvent être déchiffrés. Activez uniquement les hôtes nécessaires.",
        "de-DE": "Der private Schlüssel des gefälschten CA wird im Klartext in der lokalen Datenbank gespeichert; bei einem Leak können whitelisted HTTPS entschlüsselt werden. Aktivieren Sie nur notwendige Hosts.",
        "ru-RU": "Приватный ключ поддельного УЦ хранится в открытом виде в локальной БД; при утечке HTTPS из белого списка может быть расшифрован. Включайте только необходимые хосты.",
        "ja-JP": "偽 CA の秘密鍵はローカル DB に平文で保存されます；漏洩するとホワイトリスト内の HTTPS が復号される可能性があります。必要なホストのみ有効にしてください。",
        "es-ES": "La clave privada del CA falso se almacena en texto plano en la base de datos local; si se filtra, el HTTPS en la lista blanca puede ser descifrado. Habilite solo los hosts necesarios.",
    },
    "mitm.caCard": {
        "zh-CN": "假根证书 CA",
        "en-US": "Fake Root CA",
        "ar-SA": "CA وهمية جذرية",
        "fr-FR": "Faux CA racine",
        "de-DE": "Gefälschte Root-CA",
        "ru-RU": "Подданный корневой УЦ",
        "ja-JP": "偽ルート CA",
        "es-ES": "CA raíz falso",
    },
    "mitm.caCardDesc": {
        "zh-CN": "装到系统信任库后，客户端才会信任 AirDog 签的 host 证书",
        "en-US": "After installing to the system trust store, clients will trust host certificates signed by AirDog",
        "ar-SA": "بعد التثبيت في مخزن الثقة بالنظام، سيثق العملاء بشهادات المضيف الموقعة من AirDog",
        "fr-FR": "Après installation dans le magasin de confiance système, les clients feront confiance aux certificats d'hôte signés par AirDog",
        "de-DE": "Nach der Installation im System-Trust-Store vertrauen Clients den von AirDog signierten Host-Zertifikaten",
        "ru-RU": "После установки в системное хранилище доверия клиенты будут доверять сертификатам хостов, подписанным AirDog",
        "ja-JP": "システム信頼ストアにインストールすると、クライアントは AirDog が署名したホスト証明書を信頼します",
        "es-ES": "Tras instalarlo en el almacén de confianza del sistema, los clientes confiarán en los certificados de host firmados por AirDog",
    },
    "mitm.caPresent": {
        "zh-CN": "已生成：",
        "en-US": "Generated: ",
        "ar-SA": "تم الإنشاء: ",
        "fr-FR": "Généré : ",
        "de-DE": "Erstellt: ",
        "ru-RU": "Создан: ",
        "ja-JP": "生成済み: ",
        "es-ES": "Generado: ",
    },
    "mitm.caInstalled": {
        "zh-CN": "已装信任库：",
        "en-US": "Trust store installed: ",
        "ar-SA": "تم تثبيت مخزن الثقة: ",
        "fr-FR": "Installé dans le magasin : ",
        "de-DE": "Trust-Store installiert: ",
        "ru-RU": "Установлен в хранилище: ",
        "ja-JP": "信頼ストア インストール済み: ",
        "es-ES": "Instalado en almacén: ",
    },
    "mitm.fingerprint": {
        "zh-CN": "指纹：",
        "en-US": "Fingerprint: ",
        "ar-SA": "البصمة: ",
        "fr-FR": "Empreinte : ",
        "de-DE": "Fingerabdruck: ",
        "ru-RU": "Отпечаток: ",
        "ja-JP": "フィンガープリント: ",
        "es-ES": "Huella: ",
    },
    "mitm.installCa": {
        "zh-CN": "安装 CA",
        "en-US": "Install CA",
        "ar-SA": "تثبيت CA",
        "fr-FR": "Installer le CA",
        "de-DE": "CA installieren",
        "ru-RU": "Установить УЦ",
        "ja-JP": "CA をインストール",
        "es-ES": "Instalar CA",
    },
    "mitm.installedHint": {
        "zh-CN": "已装，客户端应已信任",
        "en-US": "Installed, clients should trust it",
        "ar-SA": "تم التثبيت، يجب أن يثق العملاء به",
        "fr-FR": "Installé, les clients devraient lui faire confiance",
        "de-DE": "Installiert, Clients sollten ihm vertrauen",
        "ru-RU": "Установлен, клиенты должны ему доверять",
        "ja-JP": "インストール済み、クライアントは信頼するはずです",
        "es-ES": "Instalado, los clientes deberían confiar en él",
    },
    "mitm.installFailed": {
        "zh-CN": "装信任库失败（exit={{code}}）",
        "en-US": "Install to trust store failed (exit={{code}})",
        "ar-SA": "فشل التثبيت في مخزن الثقة (exit={{code}})",
        "fr-FR": "Échec de l'installation dans le magasin (exit={{code}})",
        "de-DE": "Installation in Trust-Store fehlgeschlagen (exit={{code}})",
        "ru-RU": "Ошибка установки в хранилище (exit={{code}})",
        "ja-JP": "信頼ストアへのインストール失敗 (exit={{code}})",
        "es-ES": "Error al instalar en almacén (exit={{code}})",
    },
    "mitm.manualInstallTitle": {
        "zh-CN": "自动安装失败，请手动执行：",
        "en-US": "Auto-install failed. Run manually:",
        "ar-SA": "فشل التثبيت التلقائي. قم بالتشغيل يدويًا:",
        "fr-FR": "Échec de l'installation automatique. Exécutez manuellement :",
        "de-DE": "Auto-Installation fehlgeschlagen. Manuell ausführen:",
        "ru-RU": "Автоустановка не удалась. Выполните вручную:",
        "ja-JP": "自動インストール失敗。手動で実行してください:",
        "es-ES": "Instalación automática fallida. Ejecutar manualmente:",
    },
    "mitm.command": {
        "zh-CN": "命令：",
        "en-US": "Command: ",
        "ar-SA": "الأمر: ",
        "fr-FR": "Commande : ",
        "de-DE": "Befehl: ",
        "ru-RU": "Команда: ",
        "ja-JP": "コマンド: ",
        "es-ES": "Comando: ",
    },
    "mitm.whitelistTitle": {
        "zh-CN": "解密白名单",
        "en-US": "Decryption Whitelist",
        "ar-SA": "قائمة فك التشفير البيضاء",
        "fr-FR": "Liste blanche de déchiffrement",
        "de-DE": "Entschlüsselungs-Whitelist",
        "ru-RU": "Белый список расшифровки",
        "ja-JP": "復号ホワイトリスト",
        "es-ES": "Lista blanca de descifrado",
    },
    "mitm.whitelistDesc": {
        "zh-CN": "命中的 host 走 MITM 解密；未命中的走 P1 盲转。支持 *.domain 通配。",
        "en-US": "Matched hosts go through MITM decryption; unmatched go through P1 blind relay. Supports *.domain wildcard.",
        "ar-SA": "المضيفات المطابقة تمر عبر فك تشفير MITM؛ غير المطابقة تمر عبر الترحيل الأعمى P1. يدعم wildcard *.domain.",
        "fr-FR": "Les hôtes correspondants passent par le déchiffrement MITM ; les autres par le relais aveugle P1. Prend en charge *.domain.",
        "de-DE": "Übereinstimmende Hosts gehen durch MITM-Entschlüsselung; nicht übereinstimmende durch P1-Blind-Relay. Unterstützt *.domain-Wildcard.",
        "ru-RU": "Совпадающие хосты проходят через расшифровку MITM; остальные через P1 blind relay. Поддерживает *.domain.",
        "ja-JP": "一致するホストは MITM 復号を通ります; 一致しないものは P1 ブラインドリレーを通ります。*.domain ワイルドカード対応。",
        "es-ES": "Los hosts coincidentes pasan por descifrado MITM; los no coincidentes por relay ciego P1. Soporta comodín *.domain.",
    },
    "mitm.addPlaceholder": {
        "zh-CN": "*.anthropic.com",
        "en-US": "*.anthropic.com",
        "ar-SA": "*.anthropic.com",
        "fr-FR": "*.anthropic.com",
        "de-DE": "*.anthropic.com",
        "ru-RU": "*.anthropic.com",
        "ja-JP": "*.anthropic.com",
        "es-ES": "*.anthropic.com",
    },
    "mitm.whitelistEmpty": {
        "zh-CN": "（无白名单条目）",
        "en-US": "(no whitelist entries)",
        "ar-SA": "(لا توجد إدخالات في القائمة البيضاء)",
        "fr-FR": "(aucune entrée dans la liste blanche)",
        "de-DE": "(keine Whitelist-Einträge)",
        "ru-RU": "(нет записей в белом списке)",
        "ja-JP": "（ホワイトリスト エントリなし）",
        "es-ES": "(sin entradas en la lista blanca)",
    },
    "mitm.sourceDefault": {
        "zh-CN": "默认",
        "en-US": "Default",
        "ar-SA": "افتراضي",
        "fr-FR": "Défaut",
        "de-DE": "Standard",
        "ru-RU": "По умолчанию",
        "ja-JP": "デフォルト",
        "es-ES": "Predeterminado",
    },
    "mitm.sourceUser": {
        "zh-CN": "自定义",
        "en-US": "Custom",
        "ar-SA": "مخصص",
        "fr-FR": "Personnalisé",
        "de-DE": "Benutzerdefiniert",
        "ru-RU": "Пользовательский",
        "ja-JP": "カスタム",
        "es-ES": "Personalizado",
    },
}


def main():
    added = 0
    for loc in LOCALES:
        path = Path(f"src/locales/{loc}.json")
        # Preserve insertion order (existing files are NOT globally sorted).
        # Read raw text, parse to find positions, splice new keys into the
        # alphabetically correct slot within their top-level prefix group
        # (matches existing convention where keys cluster by first segment).
        data = json.loads(path.read_text(encoding="utf-8"))
        new = {}
        for key, translations in TRANS.items():
            if key not in data:
                new[key] = translations[loc]
        if not new:
            continue
        # Build new dict: existing keys preserved in order; new keys inserted
        # just before the first existing key that is alphabetically greater.
        # If no greater key exists, append at end.
        result = {}
        pending = dict(new)
        for k, v in data.items():
            # Flush any pending new keys that should come before k.
            flush = [nk for nk in pending if nk < k]
            for nk in sorted(flush):
                result[nk] = pending.pop(nk)
                added += 1
            result[k] = v
        # Append remaining new keys (those greater than all existing).
        for nk in sorted(pending):
            result[nk] = pending[nk]
            added += 1
        path.write_text(
            json.dumps(result, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
    print(f"added {added} key-locale pairs across {len(LOCALES)} locales")


if __name__ == "__main__":
    sys.exit(main())
