"use client";

// Champs de contenu d'un nœud : texte, image, lien, libellé. Ce sont les seuls champs du mode
// Contenu ; le mode Design y ajoute les réglages de structure (rôle, type, nouvel onglet…).

import type {
  AspectRatio,
  Document,
  Href,
  ImageSource,
  Node,
  PropsPatch,
  TextRole,
  TextRun,
} from "@deep-atelier/ir-types";
import { useTranslations } from "next-intl";
import { useState } from "react";

import type { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";

import {
  Checkbox,
  CommitInput,
  CommitNumber,
  CommitTextarea,
  Field,
  Select,
} from "./controls";

type Props = { session: EditorSession; doc: Document; node: Node };

/** Vrai si le nœud a des champs de contenu dans le mode donné. */
export function hasContentFields(node: Node, design: boolean): boolean {
  const type = node.kind.type;
  if (type === "Text" || type === "Image" || type === "Link") return true;
  if (type === "Button") return design || node.kind.label !== undefined;
  return (
    design &&
    type !== "ComponentInstance" &&
    type !== "RawCode" &&
    type !== "Slot"
  );
}

export function ContentFields(props: Props) {
  const { node } = props;
  switch (node.kind.type) {
    case "Text":
      return <TextFields {...props} kind={node.kind} />;
    case "Image":
      return <ImageFields {...props} kind={node.kind} />;
    case "Link":
      return <LinkFields {...props} kind={node.kind} />;
    case "Button":
      return <ButtonFields {...props} kind={node.kind} />;
    case "Icon":
      return <IconFields {...props} kind={node.kind} />;
    case "Input":
      return <InputFields {...props} kind={node.kind} />;
    case "Box":
    case "Stack":
    case "Grid":
      return <ContainerFields {...props} kind={node.kind} />;
    default:
      return null;
  }
}

function useDesign(): boolean {
  return useEditor((state) => state.mode === "design");
}

/** Applique un patch de propriétés au nœud. */
function setProps(props: Props, label: string, patch: PropsPatch): boolean {
  return props.session.edit(label, [
    { op: "set_props", node: props.node.id, props: patch },
  ]);
}

type KindOf<T extends Node["kind"]["type"]> = Extract<
  Node["kind"],
  { type: T }
>;

const TEXT_ROLES = [
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "Paragraph",
  "Inline",
  "Caption",
  "Quote",
  "Code",
  "Label",
] as const;
type TextRoleKey = (typeof TEXT_ROLES)[number];

function roleKey(role: TextRole): TextRoleKey {
  return role.kind === "Heading" ? role.level : role.kind;
}

function roleOf(key: TextRoleKey, current: TextRole): TextRole {
  switch (key) {
    case "h1":
    case "h2":
    case "h3":
    case "h4":
    case "h5":
    case "h6":
      return { kind: "Heading", level: key };
    case "Label":
      // Le champ associé au libellé est gardé.
      return current.kind === "Label" ? current : { kind: "Label" };
    default:
      return { kind: key };
  }
}

function TextFields(props: Props & { kind: KindOf<"Text"> }) {
  const t = useTranslations("editor.contentFields");
  const design = useDesign();
  const { kind } = props;
  const runs = kind.content;
  const setRun = (index: number, text: string) => {
    const content: TextRun[] = runs.map((run, i) =>
      i === index ? { ...run, text } : run,
    );
    setProps(props, "Edit text", { type: "Text", content });
  };
  return (
    <>
      {runs.length <= 1 ? (
        <Field label={t("text")}>
          <CommitTextarea
            value={runs[0]?.text ?? ""}
            onCommit={(text) =>
              setProps(props, "Edit text", {
                type: "Text",
                content: [{ ...runs[0], text }],
              })
            }
          />
        </Field>
      ) : (
        // Un texte mis en forme garde ses segments (gras, italique, couleur) : chacun s'édite.
        runs.map((run, index) => (
          <Field
            key={index}
            label={t("textRun", {
              index: index + 1,
              format: run.strong
                ? "strong"
                : run.em
                  ? "em"
                  : run.code
                    ? "code"
                    : "plain",
            })}
          >
            <CommitTextarea
              rows={2}
              value={run.text}
              onCommit={(text) => setRun(index, text)}
            />
          </Field>
        ))
      )}
      {design ? (
        <Field label={t("textRole")}>
          <Select
            value={roleKey(kind.role)}
            onChange={(event) =>
              setProps(props, "Change text role", {
                type: "Text",
                role: roleOf(event.target.value as TextRoleKey, kind.role),
              })
            }
          >
            {TEXT_ROLES.map((role) => (
              <option key={role} value={role}>
                {t(`textRoles.${role}`)}
              </option>
            ))}
          </Select>
        </Field>
      ) : null}
    </>
  );
}

const RATIOS: AspectRatio[] = [
  "auto",
  "square",
  "video",
  "portrait",
  "landscape",
  "wide",
];

/** Dimensions intrinsèques d'une image distante, lues en la chargeant dans le navigateur. */
function measure(
  url: string,
): Promise<{ width: number; height: number } | null> {
  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () =>
      resolve(
        image.naturalWidth > 0 && image.naturalHeight > 0
          ? { width: image.naturalWidth, height: image.naturalHeight }
          : null,
      );
    image.onerror = () => resolve(null);
    image.src = url;
  });
}

function ImageFields(props: Props & { kind: KindOf<"Image"> }) {
  const t = useTranslations("editor.contentFields");
  const design = useDesign();
  const { kind } = props;
  const source = kind.source;
  const [measuring, setMeasuring] = useState(false);

  const setUrl = async (raw: string) => {
    const url = raw.trim();
    if (url === "") return;
    setMeasuring(true);
    const measured = await measure(url);
    setMeasuring(false);
    const intrinsic = measured ?? kind.intrinsic;
    if (!intrinsic) {
      // Sans dimensions, l'image décalerait la mise en page à son chargement : l'IR les exige.
      useEditor.getState().setNotice("IMAGE_UNREACHABLE");
      return;
    }
    setProps(props, "Change image", {
      type: "Image",
      source: { kind: "Url", url },
      intrinsic,
    });
  };
  const setSource = (next: ImageSource) =>
    setProps(props, "Change image", { type: "Image", source: next });

  return (
    <>
      <Field label={t("imageSource")}>
        <Select
          value={source.kind}
          onChange={(event) => {
            if (event.target.value === "Placeholder") {
              setProps(props, "Change image", {
                type: "Image",
                source: {
                  kind: "Placeholder",
                  label: kind.alt || t("placeholderDefault"),
                  ratio: "video",
                },
                intrinsic: null,
              });
            }
          }}
        >
          <option value="Url">{t("imageUrl")}</option>
          <option value="Placeholder">{t("imagePlaceholder")}</option>
          {source.kind === "Asset" ? (
            <option value="Asset">{t("imageAsset")}</option>
          ) : null}
        </Select>
      </Field>
      {source.kind === "Placeholder" ? (
        <>
          <Field label={t("placeholderLabel")}>
            <CommitInput
              value={source.label}
              onCommit={(label) => setSource({ ...source, label })}
            />
          </Field>
          <Field label={t("placeholderRatio")}>
            <Select
              value={source.ratio}
              onChange={(event) =>
                setSource({
                  ...source,
                  ratio: event.target.value as AspectRatio,
                })
              }
            >
              {RATIOS.map((ratio) => (
                <option key={ratio} value={ratio}>
                  {ratio}
                </option>
              ))}
            </Select>
          </Field>
        </>
      ) : null}
      {source.kind !== "Asset" ? (
        <Field label={t("imageUrlField")}>
          <CommitInput
            type="url"
            inputMode="url"
            placeholder="https://"
            aria-busy={measuring}
            value={source.kind === "Url" ? source.url : ""}
            onCommit={(url) => void setUrl(url)}
          />
        </Field>
      ) : null}
      {source.kind === "Url" && kind.intrinsic ? (
        <div className="grid grid-cols-2 gap-2">
          <Field label={t("imageWidth")}>
            <CommitNumber
              min={1}
              max={20000}
              value={kind.intrinsic.width}
              onCommit={(width) =>
                setProps(props, "Change image size", {
                  type: "Image",
                  intrinsic: { width, height: kind.intrinsic?.height ?? width },
                })
              }
            />
          </Field>
          <Field label={t("imageHeight")}>
            <CommitNumber
              min={1}
              max={20000}
              value={kind.intrinsic.height}
              onCommit={(height) =>
                setProps(props, "Change image size", {
                  type: "Image",
                  intrinsic: { width: kind.intrinsic?.width ?? height, height },
                })
              }
            />
          </Field>
        </div>
      ) : null}
      <Field label={t("imageAlt")}>
        <CommitInput
          value={kind.alt}
          onCommit={(alt) =>
            setProps(props, "Edit alt text", { type: "Image", alt })
          }
        />
      </Field>
      <p className="text-xs text-muted-foreground">{t("imageAltHint")}</p>
      {design ? (
        <Checkbox
          label={t("imagePriority")}
          checked={kind.priority}
          onChange={(priority) =>
            setProps(props, "Change image priority", {
              type: "Image",
              priority,
            })
          }
        />
      ) : null}
    </>
  );
}

const HREF_KINDS: Href["kind"][] = [
  "Page",
  "Anchor",
  "External",
  "Email",
  "Phone",
];

function HrefFields({
  doc,
  href,
  onChange,
}: {
  doc: Document;
  href: Href;
  onChange: (href: Href) => void;
}) {
  const t = useTranslations("editor.contentFields");
  const firstPage = doc.pages[0]?.id;
  const switchKind = (kind: Href["kind"]) => {
    if (kind === href.kind) return;
    switch (kind) {
      case "Page":
        if (firstPage) onChange({ kind, page: firstPage });
        break;
      case "Anchor":
        onChange({ kind, anchor: "" });
        break;
      case "External":
        onChange({ kind, url: "https://" });
        break;
      case "Email":
        onChange({ kind, address: "" });
        break;
      case "Phone":
        onChange({ kind, number: "" });
        break;
    }
  };
  return (
    <>
      <Field label={t("linkKind")}>
        <Select
          value={href.kind}
          onChange={(event) => switchKind(event.target.value as Href["kind"])}
        >
          {HREF_KINDS.map((kind) => (
            <option key={kind} value={kind}>
              {t(`linkKinds.${kind}`)}
            </option>
          ))}
        </Select>
      </Field>
      {href.kind === "Page" ? (
        <>
          <Field label={t("linkPage")}>
            <Select
              value={href.page}
              onChange={(event) =>
                onChange({ kind: "Page", page: event.target.value })
              }
            >
              {doc.pages.map((page) => (
                <option key={page.id} value={page.id}>
                  {page.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label={t("linkAnchorOptional")}>
            <CommitInput
              value={href.anchor ?? ""}
              onCommit={(anchor) =>
                onChange(
                  anchor.trim() === ""
                    ? { kind: "Page", page: href.page }
                    : { kind: "Page", page: href.page, anchor: anchor.trim() },
                )
              }
            />
          </Field>
        </>
      ) : null}
      {href.kind === "Anchor" ? (
        <Field label={t("linkAnchor")}>
          <CommitInput
            value={href.anchor}
            onCommit={(anchor) =>
              onChange({ kind: "Anchor", anchor: anchor.trim() })
            }
          />
        </Field>
      ) : null}
      {href.kind === "External" ? (
        <Field label={t("linkUrl")}>
          <CommitInput
            type="url"
            inputMode="url"
            value={href.url}
            onCommit={(url) => onChange({ kind: "External", url: url.trim() })}
          />
        </Field>
      ) : null}
      {href.kind === "Email" ? (
        <Field label={t("linkEmail")}>
          <CommitInput
            type="email"
            inputMode="email"
            value={href.address}
            onCommit={(address) =>
              onChange({ kind: "Email", address: address.trim() })
            }
          />
        </Field>
      ) : null}
      {href.kind === "Phone" ? (
        <Field label={t("linkPhone")}>
          <CommitInput
            type="tel"
            inputMode="tel"
            value={href.number}
            onCommit={(number) =>
              onChange({ kind: "Phone", number: number.trim() })
            }
          />
        </Field>
      ) : null}
    </>
  );
}

function LinkFields(props: Props & { kind: KindOf<"Link"> }) {
  const t = useTranslations("editor.contentFields");
  const design = useDesign();
  const { kind } = props;
  return (
    <>
      {kind.label !== undefined ? (
        <Field label={t("label")}>
          <CommitInput
            value={kind.label}
            onCommit={(label) =>
              setProps(props, "Edit link label", { type: "Link", label })
            }
          />
        </Field>
      ) : null}
      <HrefFields
        doc={props.doc}
        href={kind.href}
        onChange={(href) =>
          setProps(props, "Change link target", { type: "Link", href })
        }
      />
      {design ? (
        <Checkbox
          label={t("linkNewTab")}
          checked={kind.new_tab}
          onChange={(new_tab) =>
            setProps(props, "Change link target", { type: "Link", new_tab })
          }
        />
      ) : null}
    </>
  );
}

function ButtonFields(props: Props & { kind: KindOf<"Button"> }) {
  const t = useTranslations("editor.contentFields");
  const design = useDesign();
  const { kind } = props;
  return (
    <>
      {kind.label !== undefined ? (
        <Field label={t("label")}>
          <CommitInput
            value={kind.label}
            onCommit={(label) =>
              setProps(props, "Edit button label", { type: "Button", label })
            }
          />
        </Field>
      ) : null}
      {design ? (
        <>
          <Field label={t("buttonType")}>
            <Select
              value={kind.button_type}
              onChange={(event) =>
                setProps(props, "Change button type", {
                  type: "Button",
                  button_type: event.target.value as typeof kind.button_type,
                })
              }
            >
              {(["button", "submit", "reset"] as const).map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </Select>
          </Field>
          <Checkbox
            label={t("buttonDisabled")}
            checked={kind.disabled}
            onChange={(disabled) =>
              setProps(props, "Change button state", {
                type: "Button",
                disabled,
              })
            }
          />
        </>
      ) : null}
    </>
  );
}

function IconFields(props: Props & { kind: KindOf<"Icon"> }) {
  const t = useTranslations("editor.contentFields");
  return (
    <>
      <Field label={t("iconName")}>
        <CommitInput
          spellCheck={false}
          value={props.kind.name}
          onCommit={(name) =>
            setProps(props, "Change icon", { type: "Icon", name: name.trim() })
          }
        />
      </Field>
      <p className="text-xs text-muted-foreground">{t("iconHint")}</p>
    </>
  );
}

const INPUT_TYPES = [
  "text",
  "email",
  "password",
  "number",
  "tel",
  "url",
  "search",
  "multiline",
] as const;

function InputFields(props: Props & { kind: KindOf<"Input"> }) {
  const t = useTranslations("editor.contentFields");
  const { kind } = props;
  return (
    <>
      <Field label={t("inputType")}>
        <Select
          value={kind.input_type}
          onChange={(event) =>
            setProps(props, "Change input type", {
              type: "Input",
              input_type: event.target.value as typeof kind.input_type,
            })
          }
        >
          {INPUT_TYPES.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </Select>
      </Field>
      <Field label={t("inputName")}>
        <CommitInput
          spellCheck={false}
          value={kind.name}
          onCommit={(name) =>
            setProps(props, "Rename input", {
              type: "Input",
              name: name.trim(),
            })
          }
        />
      </Field>
      <Field label={t("inputPlaceholder")}>
        <CommitInput
          value={kind.placeholder ?? ""}
          onCommit={(placeholder) =>
            setProps(props, "Edit placeholder", {
              type: "Input",
              placeholder: placeholder === "" ? null : placeholder,
            })
          }
        />
      </Field>
      <Checkbox
        label={t("inputRequired")}
        checked={kind.required}
        onChange={(required) =>
          setProps(props, "Change input", { type: "Input", required })
        }
      />
    </>
  );
}

const CONTAINER_ROLES = [
  "Generic",
  "Section",
  "Header",
  "Footer",
  "Nav",
  "Main",
  "Article",
  "Aside",
  "List",
  "ListItem",
  "Form",
] as const;

function ContainerFields(
  props: Props & { kind: KindOf<"Box"> | KindOf<"Stack"> | KindOf<"Grid"> },
) {
  const t = useTranslations("editor.contentFields");
  const { kind } = props;
  return (
    <Field label={t("containerRole")}>
      <Select
        value={kind.role.kind}
        onChange={(event) => {
          const next = event.target.value as (typeof CONTAINER_ROLES)[number];
          setProps(props, "Change role", {
            type: kind.type,
            role:
              next === "Form"
                ? kind.role.kind === "Form"
                  ? kind.role
                  : { kind: "Form", method: "post" }
                : { kind: next },
          });
        }}
      >
        {CONTAINER_ROLES.map((role) => (
          <option key={role} value={role}>
            {t(`containerRoles.${role}`)}
          </option>
        ))}
      </Select>
    </Field>
  );
}
