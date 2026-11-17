<div align="center">

# Zapret2 OpenWRT

## 💻 Установка

Подключитесь к роутеру по SSH и выполните:


```sh
wget -qO- https://raw.githubusercontent.com/alex0310/zapret2-openwrt/main/install-zapret2.sh | sh
```

Установщик скопирует файлы в систему роутера и настроит службы.

## 🔑 Ключ подписи APK (OpenWrt 25.12.6)

Пакеты APK подписаны ключом zapret2. Установщик скачивает и кладёт ключ в
`/etc/apk/keys/` автоматически. При ручной установке сделайте это сами:

```sh
wget -O /etc/apk/keys/zapret2-apk-key.pem \
  https://github.com/alex0310/zapret2-openwrt/releases/download/v1.0.1-r1/zapret2-apk-key.pem
```

После этого локальные пакеты можно ставить с проверкой подписи, без
`--allow-untrusted`. Ключ также лежит в репозитории:
`apk-keys/zapret2-apk-key.pem`.
