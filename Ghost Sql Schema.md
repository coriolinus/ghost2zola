# Ghost Sql Schema

```sql
CREATE TABLE IF NOT EXISTS "posts" (
    "id" integer not null primary key autoincrement,
    "uuid" varchar(36) not null,
    "title" varchar(150) not null,
    "slug" varchar(150) not null,
    "markdown" text null,
    "mobiledoc" text null,
    "html" text null,
    "amp" text null,
    "image" text null,
    "featured" boolean not null default '0',
    "page" boolean not null default '0',
    "status" varchar(150) not null default 'draft',
    "language" varchar(6) not null default 'en_US',
    "visibility" varchar(150) not null default 'public',
    "meta_title" varchar(150) null,
    "meta_description" varchar(200) null,
    "author_id" integer not null,
    "created_at" datetime not null,
    "created_by" integer not null,
    "updated_at" datetime null,
    "updated_by" integer null,
    "published_at" datetime null,
    "published_by" integer null
);

CREATE TABLE IF NOT EXISTS "users" (
    "id" integer not null primary key autoincrement,
    "uuid" varchar(36) not null,
    "name" varchar(150) not null,
    "slug" varchar(150) not null,
    "password" varchar(60) not null,
    "email" varchar(254) not null,
    "image" text null,
    "cover" text null,
    "bio" varchar(200) null,
    "website" text null,
    "location" text null,
    "facebook" text null,
    "twitter" text null,
    "accessibility" text null,
    "status" varchar(150) not null default 'active',
    "language" varchar(6) not null default 'en_US',
    "visibility" varchar(150) not null default 'public',
    "meta_title" varchar(150) null,
    "meta_description" varchar(200) null,
    "tour" text null,
    "last_login" datetime null,
    "created_at" datetime not null,
    "created_by" integer not null,
    "updated_at" datetime null,
    "updated_by" integer null
);

CREATE TABLE IF NOT EXISTS "tags" (
    "id" integer not null primary key autoincrement,
    "uuid" varchar(36) not null,
    "name" varchar(150) not null,
    "slug" varchar(150) not null,
    "description" varchar(200) null,
    "image" text null,
    "parent_id" integer null,
    "visibility" varchar(150) not null default 'public',
    "meta_title" varchar(150) null,
    "meta_description" varchar(200) null,
    "created_at" datetime not null,
    "created_by" integer not null,
    "updated_at" datetime null,
    "updated_by" integer null
);

CREATE TABLE IF NOT EXISTS "posts_tags" (
    "id" integer not null primary key autoincrement,
    "post_id" integer not null,
    "tag_id" integer not null,
    "sort_order" integer not null default '0',
    foreign key("post_id") references "posts"("id"), foreign key("tag_id") references "tags"("id")
);
```

## Examples

```sql
sqlite> select posts.id, posts.title, posts.status, users.name, posts.published_at, substr(posts.markdown, 0, 10) from posts inner join users on posts.author_id = users.id order by posts.id desc limit 3;
id          title            status      name        published_at         substr(posts.markdown, 0, 10)
----------  ---------------  ----------  ----------  -------------------  -----------------------------
95          Die Wasserratte  published   Pete        2020-09-30 20:17:00  Christina
94          Reading          published   Pete        2020-09-16 09:24:30  This morn
93          the next iterat  draft       Pete        2020-11-10 08:55:00  ![](/cont

sqlite> select posts.title, tags.name from posts inner join posts_tags on posts.id = posts_tags.post_id inner join tags on posts_tags.tag_id = tags.id;
```
