# nfs-mamont

***
##  С чего начать?
Для того, чтобы облегчить знакомство с сервисом GitFlic и первые шаги в нём, мы подготовили несколько рекомендаций.  
Уже опытный пользователь? Отредактируйте данный **README** файл по своему усмотрению.  
Не знаете что добавить в него? Перейдите в раздел `"Что должен содержать README файл"`, в котором описаны ключевые компоненты хорошего README файла. 

## Добавьте свои файлы
Если вы решили начать разработку проекта с создания репозитория в нашем сервисе, тогда клонируйте себе данный репозиторий следующим образом:


```
git clone https://gitflic.ru/project/yadro/nfs-mamont.git
cd nfs-mamont
**добавьте первые файлы вашего проекта**
git add .
git commit -m "Первый коммит"
git push -u origin master
```

Уже что-то делали в проекте? В таком случае инициализируйте гит-репозиторий в корне проекта и добавьте текущий репозиторий как удалённый репозиторий:

```
cd existing_folder
git init
git remote add origin https://gitflic.ru/project/yadro/nfs-mamont.git
git clone
**добавьте новые файлы**
git add .
git commit -m "Новый коммит"
git push -u origin master
```
***


# Что должен содержать README файл


Прежде всего, стоит понимать, что `README.md` — это краткая документация. Это первое, что видит человек, который открывает репозиторий. Поэтому здесь важно дать достаточно информации о проекте и рассказать, что он из себя представляет.
Ключевая информация, которую должен содержать README файл:

## Название и описание
Название проекта должно быть простым и понятным (чаще всего это одно слово).
Описание должно описывать основные функции проекта, включая его особенности и назначение. 
Если у вашего проекта есть альтернативные проекты, то в описании можно перечислить ключевые отличия, которые выделяют ваш проект на фоне всех остальных.

## Установка и настройка
Также в `README` файле рекомендуется перечислить необходимые инструкции для установки, 
будь то использование пакетных менеджеров (например, `Homebrew` на MacOS или `apt` на Linux), 
зависимости, которые могут понадобиться в ходе использования, а также шаги по их настройке.

## Совместная разработка
Можно добавить информацию о том, как принять участие в разработке вашего проекта, как стать непосредственным участником, правила оформления pull-requests и т.д.

## Контакты
Ссылки на внешние ресурсы, такие как документация, блог, страница проекта в социальных сетях, сообщество проекта и т.д.

## Статус проекта
В данном разделе рекомендуется указывать, на какой стадии находится проект, активно разрабатывается или находится в стадии застоя.
Если же проект готов и во всю используется, можно указывать актуальную версию, а также последние изменения, которые были сделаны с момента предыдущего релиза.

***

# Полезные ссылки

***

## Работа с проектом

- [ ] [Как создать проект](https://docs.gitflic.ru/project/project_create)
- [ ] [Как импортировать проект](https://docs.gitflic.ru/project/import_base)
- [ ] [Запросы на слияние](https://docs.gitflic.ru/project/merge_request)
- [ ] [Зеркалирование проекта](https://docs.gitflic.ru/project/mirror)
- [ ] [Импортировать проект с GitLab](https://docs.gitflic.ru/project/import)

## Команды
- [ ] [Создание команды](https://docs.gitflic.ru/team/create)
- [ ] [Обзор команды](https://docs.gitflic.ru/team/view)
- [ ] [Настройка команды](https://docs.gitflic.ru/team/settings)

## Реестр пакетов
- [ ] [Реестр пакетов](https://docs.gitflic.ru/registry/package)
- [ ] [PyPi](https://docs.gitflic.ru/registry/pypi_registry)
- [ ] [Generic](https://docs.gitflic.ru/registry/generic_registry)
- [ ] [Maven](https://docs.gitflic.ru/registry/maven_registry)
- [ ] [Docker](https://docs.gitflic.ru/registry/docker)

## Компании
- [ ] [Создание компании](https://docs.gitflic.ru/company/create)
- [ ] [Обзор компании](https://docs.gitflic.ru/company/view)
- [ ] [Тарифы и оплата](https://docs.gitflic.ru/company/price)
- [ ] [Запуск агента компании](https://docs.gitflic.ru/company/saas_runner_setup)

## CI/CD
- [ ] [Что такое GitFlic CI/CD](https://docs.gitflic.ru/cicd/introduction)
- [ ] [Задача (Job)](https://docs.gitflic.ru/cicd/job)
- [ ] [Конвейер (pipeline)](https://docs.gitflic.ru/cicd/pipeline)
- [ ] [Агенты](https://docs.gitflic.ru/cicd/agent)
- [ ] [Справочник для .yaml файла](https://docs.gitflic.ru/cicd/gitflic-ci-yaml)

## API
- [ ] [Введение в GitFlic API](https://docs.gitflic.ru/api/intro)
- [ ] [Методы для администратора](https://docs.gitflic.ru/api/admin)
- [ ] [Получение access токена](https://docs.gitflic.ru/api/access-token)


## Панель администратора
- [ ] [Панель администратора](https://docs.gitflic.ru/admin_panel/intro)
- [ ] [Панель управления](https://docs.gitflic.ru/admin_panel/dashboard)
- [ ] [Настройка LDAP](https://docs.gitflic.ru/admin_panel/ldap)
- [ ] [Ключевые настройки](https://docs.gitflic.ru/admin_panel/settings)

## Общая информация
- [ ] [Глоссарий](https://docs.gitflic.ru/common/gloss)
- [ ] [Права доступа ролей](https://docs.gitflic.ru/common/manage_roles)
- [ ] [Вебхуки](https://docs.gitflic.ru/common/webhook)