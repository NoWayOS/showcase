// The Screen Compiler gallery, 9 of 11: one child seen through a view that scrolls - with a mouse's wheel,
// a drag, the keys or its scrollbar - starting with paragraphs the application sets, wrapped onto as many
// lines as they need; a row that scrolls across; and, inside the view, panels that scroll down and both
// ways.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Card, Column, HeaderBar, Row, Screen, ScrollView, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;
const STORY_BYTES = 2048;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
  story: string;
};

export function GalleryScrollScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Scrolling
          </Text>
          <Spacer />
          <Text>
            9 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={GAP} style={{ overflowX: "auto" }}>
          <Card padding={TIGHT}><Text>Monday</Text></Card>
          <Card padding={TIGHT}><Text>Tuesday</Text></Card>
          <Card padding={TIGHT}><Text>Wednesday</Text></Card>
          <Card padding={TIGHT}><Text>Thursday</Text></Card>
          <Card padding={TIGHT}><Text>Friday</Text></Card>
          <Card padding={TIGHT}><Text>Saturday</Text></Card>
          <Card padding={TIGHT}><Text>Sunday</Text></Card>
          <Card padding={TIGHT}><Text>Next Monday</Text></Card>
          <Card padding={TIGHT}><Text>Next Tuesday</Text></Card>
        </Row>
        <Text role="caption">{'<Row style={{ overflowX: "auto" }}>: across; <ScrollView><Column>...</Column></ScrollView> below: down'}</Text>
        <ScrollView id="lines">
          <Column gap={GAP}>
            <Text maxLength={STORY_BYTES}>{props.story}</Text>
            <Text role="caption">{'<Text maxLength={2048}>{props.story}</Text>: paragraphs the application sets'}</Text>
            <Row gap={GAP}>
              <Column gap={4} style={{ overflowY: "scroll", width: 240, height: 96 }}>
                <Text>Down 1</Text>
                <Text>Down 2</Text>
                <Text>Down 3</Text>
                <Text>Down 4</Text>
                <Text>Down 5</Text>
                <Text>Down 6</Text>
              </Column>
              <Column style={{ overflow: "auto", width: 240, height: 96 }}>
                <Row gap={GAP} style={{ width: 480 }}>
                  <Column gap={4}>
                    <Text>Both ways 1</Text>
                    <Text>Both ways 2</Text>
                    <Text>Both ways 3</Text>
                    <Text>Both ways 4</Text>
                    <Text>Both ways 5</Text>
                  </Column>
                  <Column gap={4}>
                    <Text>and across 1</Text>
                    <Text>and across 2</Text>
                    <Text>and across 3</Text>
                    <Text>and across 4</Text>
                    <Text>and across 5</Text>
                  </Column>
                </Row>
              </Column>
            </Row>
            <Text role="caption">{'<Column style={{ overflowY: "scroll" }}>, and overflow: "auto" both ways, inside the view'}</Text>
            <Text>Line 1: the top</Text>
            <Text>Line 2</Text>
            <Text>Line 3</Text>
            <Text>Line 4</Text>
            <Text>Line 5</Text>
            <Text>Line 6</Text>
            <Text>Line 7</Text>
            <Text>Line 8</Text>
            <Text>Line 9</Text>
            <Text>Line 10</Text>
            <Text>Line 11</Text>
            <Text>Line 12</Text>
            <Text>Line 13</Text>
            <Text>Line 14</Text>
            <Text>Line 15</Text>
            <Text>Line 16: the end</Text>
          </Column>
        </ScrollView>
      </Column>
    </Screen>
  );
}
