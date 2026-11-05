// The Screen Compiler gallery, 11 of 11: the style properties a screen may declare: sizes, alignment, and
// the theme's colours by name - never a colour value, so the
// large-text, high-contrast theme recolours them - the theme's shadows, gradients and opacity, and
// text a press selects for Ctrl+C to copy.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Card, Column, HeaderBar, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
};

export function GalleryCssScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Declared CSS
          </Text>
          <Spacer />
          <Text>
            11 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext} style={{ cursor: "pointer" }}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={EDGE}>
          <Card
            style={{
              width: 280,
              height: 120,
              gap: 4,
              padding: 12,
              alignItems: "center",
              justifyContent: "center",
              backgroundColor: "input",
              borderColor: "focus",
              borderWidth: 3,
              borderRadius: 24,
            }}
          >
            <Text style={{ color: "text-muted", textAlign: "center", userSelect: "text", cursor: "text" }}>
              Centred, muted
            </Text>
            <Text style={{ color: "error", textAlign: "center" }}>In the error colour</Text>
          </Card>
          <Column gap={4}>
            <Text role="caption">{'<Card style={{ width: 280, height: 120,'}</Text>
            <Text role="caption">{'  alignItems: "center", justifyContent: "center",'}</Text>
            <Text role="caption">{'  backgroundColor: "input", borderColor: "focus",'}</Text>
            <Text role="caption">{'  borderWidth: 3, borderRadius: 24 }}>'}</Text>
            <Text role="caption">{'<Text style={{ color: "text-muted", textAlign: "center",'}</Text>
            <Text role="caption">{'  userSelect: "text", cursor: "text" }}>: press it, Ctrl+C copies it'}</Text>
          </Column>
        </Row>
        <Row style={{ minWidth: 320, maxWidth: 560, justifyContent: "space-between" }}>
          <Text>Spread</Text>
          <Text>between</Text>
          <Text>320 and 560</Text>
        </Row>
        <Text role="caption">{'<Row style={{ minWidth: 320, maxWidth: 560, justifyContent: "space-between" }}>'}</Text>
        <Row gap={EDGE}>
          <Card padding={TIGHT} style={{ boxShadow: "low" }}>
            <Text>low</Text>
          </Card>
          <Card padding={TIGHT} style={{ boxShadow: "medium" }}>
            <Text>medium</Text>
          </Card>
          <Card padding={TIGHT} style={{ boxShadow: "high" }}>
            <Text>high</Text>
          </Card>
          <Column padding={TIGHT} style={{ background: "linear-gradient(to right, button, text)", borderRadius: 8 }}>
            <Text style={{ color: "button-text" }}>A gradient</Text>
          </Column>
          <Card padding={TIGHT} style={{ opacity: 0.5, cursor: "not-allowed" }}>
            <Text>Half see-through</Text>
          </Card>
        </Row>
        <Text role="caption">{'style={{ boxShadow: "medium" }}, { background: "linear-gradient(to right, button, text)" }, { opacity: 0.5, cursor: "not-allowed" }'}</Text>
        <Row gap={EDGE}>
          <Card padding={TIGHT} style={{ animation: "pulse 300ms ease-in-out 2 alternate" }}>
            <Text>pulse</Text>
          </Card>
          <Card padding={TIGHT} style={{ animationName: "shimmer", animationDuration: "0.5s", animationTimingFunction: "linear", animationDelay: "0ms", animationIterationCount: "1", animationDirection: "normal", animationFillMode: "none" }}>
            <Text>shimmer</Text>
          </Card>
          <Card padding={TIGHT} style={{ transition: "opacity 250ms ease-out" }}>
            <Text>fades in when shown</Text>
          </Card>
          <Card padding={TIGHT} style={{ transitionProperty: "transform", transitionDuration: "0.3s", transitionTimingFunction: "cubic-bezier(0.2, 0, 0, 1)", transitionDelay: "50ms" }}>
            <Text>rises in when shown</Text>
          </Card>
        </Row>
        <Text role="caption">{'style={{ animation: "pulse 300ms ease-in-out 2 alternate" }}, { transition: "opacity 250ms ease-out" }, and their longhands'}</Text>
      </Column>
    </Screen>
  );
}
